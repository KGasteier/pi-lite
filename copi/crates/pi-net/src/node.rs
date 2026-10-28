//! Direkter iroh-QUIC-Transport ohne Relays/Discovery. TLS bestimmt den Absender.
//! Ein serieller Versandarbeiter je Peer erhält die Reihenfolge auch bei Wiederholung.
use crate::{
    identity::{peer_of, Identity},
    model::{MsgId, PeerId},
    wire::{Envelope, MAX_ENVELOPE},
};
use iroh::{
    endpoint::{presets, Connection},
    Endpoint, EndpointAddr,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc as channel, Notify},
    task::JoinSet,
};

pub const ALPN: &[u8] = b"cocopi/1";
pub const RATE_PER_MIN: u32 = 120;
pub const MAX_QUEUE: usize = 200;
const STREAM_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONNECTIONS: usize = 128;

#[derive(Clone, Debug)]
pub struct Outgoing {
    pub to: PeerId,
    pub env: Envelope,
    pub tag: Option<MsgId>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum NodeEvent {
    Address(String),
    Received {
        from: PeerId,
        env: Envelope,
    },
    Delivered {
        to: PeerId,
        tag: Option<MsgId>,
    },
    Failed {
        to: PeerId,
        tag: Option<MsgId>,
        error: String,
    },
    Rejected {
        from: PeerId,
        why: String,
    },
    Reachable {
        peer: PeerId,
        ok: bool,
    },
    Error(String),
    Stopped,
}
enum Command {
    Allowed(Vec<(PeerId, String)>),
    Send(Outgoing, Vec<u8>, QueueSlot),
    Stop,
}
/// Die Reservierung zählt Kanal, Peer-Warteschlange und laufende Übertragung zusammen.
struct QueueSlot(Arc<AtomicUsize>);
impl Drop for QueueSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
#[derive(Default)]
struct State {
    allowed: HashMap<PeerId, EndpointAddr>,
    connections: HashMap<PeerId, Vec<Connection>>,
    rates: HashMap<PeerId, VecDeque<Instant>>,
    activity: HashMap<usize, Instant>,
    reachable: HashMap<PeerId, bool>,
    ausweis: Option<crate::ausweis::Ausweis>,
}
type Shared = Arc<Mutex<State>>;

pub struct Node {
    pub me: PeerId,
    pub events: mpsc::Receiver<NodeEvent>,
    tx: channel::Sender<Command>,
    out: mpsc::Sender<NodeEvent>,
    queued: Arc<AtomicUsize>,
    stopping: Arc<AtomicBool>,
    wake: Arc<Notify>,
}
impl Node {
    /// Start nur nach ausdrücklichem Klick; eigene Laufzeit in einem eigenen Thread.
    pub fn start(identity: Identity) -> Node {
        Self::start_with_ausweis(identity, None)
    }
    /// Optionaler A6-Ausweis; Austausch erfolgt auf jeder neuen Verbindung.
    /// Prüfung/Vertrauensanker bleiben bei der Anwendung, nicht beim Transport.
    pub fn start_with_ausweis(
        identity: Identity,
        ausweis: Option<crate::ausweis::Ausweis>,
    ) -> Node {
        let me = identity.peer();
        let (tx, rx) = channel::channel(MAX_QUEUE);
        let (out, events) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Notify::new());
        let node = Node {
            me,
            events,
            tx,
            out: out.clone(),
            queued: Arc::new(AtomicUsize::new(0)),
            stopping: stopping.clone(),
            wake: wake.clone(),
        };
        let failures = out.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("cocopi-netz".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("Netz-Laufzeit: {e}"))
                    .and_then(|rt| {
                        rt.block_on(run(identity, rx, out.clone(), stopping, wake, ausweis))
                    });
                if let Err(e) = result {
                    let _ = out.send(NodeEvent::Error(e));
                }
                let _ = out.send(NodeEvent::Stopped);
            })
        {
            let _ = failures.send(NodeEvent::Error(format!("Netz-Thread: {e}")));
            let _ = failures.send(NodeEvent::Stopped);
        }
        node
    }
    /// Der bestehende Vertrag liefert `()`: Kanalfehler werden als Ereignis gemeldet.
    pub fn set_allowed(&self, peers: Vec<(PeerId, String)>) {
        if let Err(e) = self.tx.try_send(Command::Allowed(peers)) {
            let _ = self.out.send(NodeEvent::Error(format!(
                "Kontaktliste nicht übernommen: {e}"
            )));
        }
    }
    pub fn send(&self, o: Outgoing) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) {
            return Err("Netz angehalten".into());
        }
        let data = o.env.encode()?;
        self.queued
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_QUEUE).then_some(n + 1)
            })
            .map_err(|_| "Warteschlange voll".to_string())?;
        let slot = QueueSlot(self.queued.clone());
        self.tx
            .try_send(Command::Send(o, data, slot))
            .map_err(|e| match e {
                channel::error::TrySendError::Full(_) => "Warteschlange voll".into(),
                channel::error::TrySendError::Closed(_) => "Netz angehalten".into(),
            })
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        // Auch bei vollem Befehlskanal zuverlässig und ohne Blockieren beenden.
        if let Err(e) = self.tx.try_send(Command::Stop) {
            let _ = self.out.send(NodeEvent::Error(format!("Stop-Befehl: {e}")));
        }
        self.wake.notify_one();
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn parse_address(addr: &str) -> Result<PeerId, String> {
    let a: EndpointAddr =
        serde_json::from_str(addr.trim()).map_err(|e| format!("Kontaktadresse ungültig: {e}"))?;
    Ok(peer_of(&a.id))
}

async fn run(
    identity: Identity,
    mut rx: channel::Receiver<Command>,
    out: mpsc::Sender<NodeEvent>,
    stopping: Arc<AtomicBool>,
    wake: Arc<Notify>,
    ausweis: Option<crate::ausweis::Ausweis>,
) -> Result<(), String> {
    let endpoint = Endpoint::builder(presets::Minimal)
        .secret_key(identity.secret)
        .alpns(vec![ALPN.to_vec()])
        .transport_config(
            iroh::endpoint::QuicTransportConfig::builder()
                .max_concurrent_bidi_streams(16u32.into())
                .max_concurrent_uni_streams(0u32.into())
                .stream_receive_window((MAX_ENVELOPE as u32 + 1).into())
                .receive_window((4 * MAX_ENVELOPE as u32).into())
                .build(),
        )
        .bind()
        .await
        .map_err(|e| format!("Netzstart: {e}"))?;
    let address =
        serde_json::to_string(&endpoint.addr()).map_err(|e| format!("Eigene Adresse: {e}"))?;
    let _ = out.send(NodeEvent::Address(address));
    let state = Arc::new(Mutex::new(State {
        ausweis,
        ..State::default()
    }));
    let mut tasks = JoinSet::new();
    let mut workers: HashMap<PeerId, channel::Sender<(Outgoing, Vec<u8>, QueueSlot)>> =
        HashMap::new();
    loop {
        if stopping.load(Ordering::Acquire) {
            break;
        }
        tokio::select! {
            biased;
            _ = wake.notified() => break,
            cmd = rx.recv() => match cmd {
                Some(Command::Allowed(peers)) => replace_allowed(&state, peers, &out),
                Some(Command::Send(o, data, slot)) => {
                    let allowed = state.lock().expect("Netzzustand").allowed.contains_key(&o.to);
                    if !allowed { failed(&out, &o, "Ziel nicht erlaubt"); continue; }
                    let worker = workers.entry(o.to.clone()).or_insert_with(|| {
                        let (tx, rx) = channel::channel(MAX_QUEUE);
                        tasks.spawn(send_worker(endpoint.clone(), state.clone(), out.clone(), rx));
                        tx
                    });
                    // Gesamtreservierung garantiert Platz, kein await in der Steuerschleife.
                    if let Err(e) = worker.try_send((o, data, slot)) {
                        let (o, _, _) = e.into_inner(); failed(&out, &o, "Versandarbeiter nicht verfügbar");
                    }
                }
                Some(Command::Stop) | None => break,
            },
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break; };
                // Handshakes dürfen weder Steuerbefehle noch andere Verbindungen aufhalten.
                if tasks.len() >= MAX_CONNECTIONS + MAX_QUEUE { incoming.refuse(); continue; }
                let state = state.clone(); let out = out.clone();
                tasks.spawn(async move {
                    match tokio::time::timeout(STREAM_TIMEOUT, incoming).await {
                        Ok(Ok(c)) => receive_connection(c, state, out).await,
                        _ => { let _ = out.send(NodeEvent::Error("Eingehende Verbindung fehlgeschlagen".into())); }
                    }
                });
            },
            _ = tasks.join_next(), if !tasks.is_empty() => {},
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    workers.clear();
    endpoint.close().await;
    Ok(())
}

fn replace_allowed(state: &Shared, peers: Vec<(PeerId, String)>, out: &mpsc::Sender<NodeEvent>) {
    let mut allowed = HashMap::new();
    for (peer, address) in peers {
        match serde_json::from_str::<EndpointAddr>(address.trim()) {
            Ok(addr) if peer_of(&addr.id) == peer => {
                // Minimal alleine verhindert nicht, dass eine fremde Kontaktadresse Relays
                // vorgibt. Daher ausschließlich direkte IP-Adressen übernehmen.
                let addr = EndpointAddr::from_parts(
                    addr.id,
                    addr.ip_addrs().copied().map(iroh::TransportAddr::Ip),
                );
                allowed.insert(peer, addr);
            }
            _ => {
                let _ = out.send(NodeEvent::Error(
                    "Kontaktadresse ungültig oder Geräte-ID stimmt nicht überein".into(),
                ));
            }
        }
    }
    let mut s = state.lock().expect("Netzzustand");
    s.connections.retain(|peer, connections| {
        if allowed.contains_key(peer) {
            connections.retain(|c| c.close_reason().is_none());
            true
        } else {
            for c in connections {
                c.close(1u32.into(), b"not allowed");
            }
            false
        }
    });
    s.rates.retain(|peer, _| allowed.contains_key(peer));
    s.reachable.retain(|peer, _| allowed.contains_key(peer));
    s.allowed = allowed;
}
fn register(c: &Connection, state: &Shared, out: &mpsc::Sender<NodeEvent>) -> bool {
    let peer = peer_of(&c.remote_id());
    let mut s = state.lock().expect("Netzzustand");
    let why = if !s.allowed.contains_key(&peer) {
        Some("Gerät nicht erlaubt")
    } else {
        let count: usize = s
            .connections
            .values_mut()
            .map(|v| {
                v.retain(|c| c.close_reason().is_none());
                v.len()
            })
            .sum();
        if count >= MAX_CONNECTIONS {
            Some("Zu viele Verbindungen")
        } else {
            None
        }
    };
    if let Some(why) = why {
        c.close(1u32.into(), b"rejected");
        let _ = out.send(NodeEvent::Rejected {
            from: peer,
            why: why.into(),
        });
        false
    } else {
        s.activity.insert(c.stable_id(), Instant::now());
        s.connections.entry(peer).or_default().push(c.clone());
        true
    }
}
fn rate_allowed(s: &mut State, peer: &PeerId, now: Instant) -> bool {
    let times = s.rates.entry(peer.clone()).or_default();
    while times
        .front()
        .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
    {
        times.pop_front();
    }
    if times.len() >= RATE_PER_MIN as usize {
        false
    } else {
        times.push_back(now);
        true
    }
}
async fn receive_connection(c: Connection, state: Shared, out: mpsc::Sender<NodeEvent>) {
    if !register(&c, &state, &out) {
        return;
    }
    receive_streams(c, state, out).await;
}
async fn receive_streams(c: Connection, state: Shared, out: mpsc::Sender<NodeEvent>) {
    let ausweis = state.lock().expect("Netzzustand").ausweis.clone();
    // Beide Seiten lesen parallel: kein Handshake-Deadlock, keine Änderung des
    // v1-Umschlags. Ohne Ausweis bleibt das bisherige Transportverhalten erhalten.
    let announce = async {
        if let Some(ausweis) = ausweis {
            let exchange = async {
                let bytes = Envelope::new(crate::wire::Payload::Geraeteausweis { ausweis })
                    .encode()
                    .map_err(|_| ())?;
                let (mut send, mut recv) = c.open_bi().await.map_err(|_| ())?;
                send.write_all(&bytes).await.map_err(|_| ())?;
                send.finish().map_err(|_| ())?;
                recv.read_to_end(16).await.map_err(|_| ())?;
                Ok::<(), ()>(())
            };
            let _ = tokio::time::timeout(STREAM_TIMEOUT, exchange).await;
        }
    };
    tokio::join!(announce, receive_loop(c.clone(), state, out));
}
async fn receive_loop(c: Connection, state: Shared, out: mpsc::Sender<NodeEvent>) {
    let peer = peer_of(&c.remote_id());
    loop {
        let (mut send, mut recv) = match tokio::time::timeout(IDLE_TIMEOUT, c.accept_bi()).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(_)) => break,
            Err(_) => {
                let last = state
                    .lock()
                    .expect("Netzzustand")
                    .activity
                    .get(&c.stable_id())
                    .copied();
                if last.is_some_and(|t| t.elapsed() < IDLE_TIMEOUT) {
                    continue;
                }
                c.close(0u32.into(), b"idle");
                break;
            }
        };
        let accepted = {
            let mut s = state.lock().expect("Netzzustand");
            s.activity.insert(c.stable_id(), Instant::now());
            s.allowed.contains_key(&peer) && rate_allowed(&mut s, &peer, Instant::now())
        };
        if !accepted {
            c.close(1u32.into(), b"rejected");
            let _ = out.send(NodeEvent::Rejected {
                from: peer.clone(),
                why: "Gerät gesperrt oder Ratenbegrenzung erreicht".into(),
            });
            break;
        }
        let process = async {
            let decoded = match recv.read_to_end(MAX_ENVELOPE).await {
                Ok(data) => Envelope::decode(&data).ok(),
                Err(_) => None,
            };
            let reply = if let Some(env) = decoded {
                // Nach dem Lesen nochmals prüfen: Widerruf darf keine verspätete Zustellung erlauben.
                if !state
                    .lock()
                    .expect("Netzzustand")
                    .allowed
                    .contains_key(&peer)
                {
                    return;
                }
                if out
                    .send(NodeEvent::Received {
                        from: peer.clone(),
                        env,
                    })
                    .is_err()
                {
                    return;
                }
                b"ok".as_slice()
            } else {
                // Keine Parserfehler mit möglicherweise vertraulichen Nutzlastteilen ausgeben.
                let _ = out.send(NodeEvent::Error("Umschlag ungültig oder zu groß".into()));
                b"err".as_slice()
            };
            let _ = send.write_all(reply).await;
            let _ = send.finish();
        };
        if tokio::time::timeout(STREAM_TIMEOUT, process).await.is_err() {
            let _ = out.send(NodeEvent::Error("Stream: Zeitlimit erreicht".into()));
            c.close(1u32.into(), b"timeout");
            break;
        }
    }
    let mut s = state.lock().expect("Netzzustand");
    s.activity.remove(&c.stable_id());
    if let Some(v) = s.connections.get_mut(&peer) {
        v.retain(|other| other.stable_id() != c.stable_id());
    }
}
fn failed(out: &mpsc::Sender<NodeEvent>, o: &Outgoing, error: &str) {
    let _ = out.send(NodeEvent::Failed {
        to: o.to.clone(),
        tag: o.tag.clone(),
        error: error.into(),
    });
}
fn reachable(state: &Shared, out: &mpsc::Sender<NodeEvent>, peer: &PeerId, ok: bool) {
    if state
        .lock()
        .expect("Netzzustand")
        .reachable
        .insert(peer.clone(), ok)
        != Some(ok)
    {
        let _ = out.send(NodeEvent::Reachable {
            peer: peer.clone(),
            ok,
        });
    }
}
async fn send_worker(
    endpoint: Endpoint,
    state: Shared,
    out: mpsc::Sender<NodeEvent>,
    mut rx: channel::Receiver<(Outgoing, Vec<u8>, QueueSlot)>,
) {
    // Empfangstasks für ausgehend aufgebaute Verbindungen werden mit dem Arbeiter beendet.
    let mut readers = JoinSet::new();
    while let Some((o, data, _slot)) = rx.recv().await {
        let mut error = "Übertragung fehlgeschlagen".to_string();
        let mut delivered = false;
        for attempt in 0..6 {
            let addr = state
                .lock()
                .expect("Netzzustand")
                .allowed
                .get(&o.to)
                .cloned();
            let Some(addr) = addr else {
                error = "Ziel nicht erlaubt".into();
                break;
            };
            let cached = state
                .lock()
                .expect("Netzzustand")
                .connections
                .get(&o.to)
                .and_then(|v| v.iter().find(|c| c.close_reason().is_none()))
                .cloned();
            let mut used = None;
            let result = tokio::time::timeout(STREAM_TIMEOUT, async {
                let c = if let Some(c) = cached {
                    c
                } else {
                    let c = endpoint
                        .connect(addr, ALPN)
                        .await
                        .map_err(|_| "Verbindung fehlgeschlagen")?;
                    // Die Empfangstask registriert vor ihrem ersten await. Hier explizit
                    // registrieren, damit Folgesendungen den Cache sofort finden.
                    if !register(&c, &state, &out) {
                        return Err("Ziel nicht erlaubt");
                    }
                    let reader = c.clone();
                    let rs = state.clone();
                    let ro = out.clone();
                    readers.spawn(async move {
                        receive_streams(reader, rs, ro).await;
                    });
                    c
                };
                used = Some(c.clone());
                state
                    .lock()
                    .expect("Netzzustand")
                    .activity
                    .insert(c.stable_id(), Instant::now());
                let (mut s, mut r) = c
                    .open_bi()
                    .await
                    .map_err(|_| "Stream konnte nicht geöffnet werden")?;
                s.write_all(&data)
                    .await
                    .map_err(|_| "Senden fehlgeschlagen")?;
                s.finish().map_err(|_| "Streamabschluss fehlgeschlagen")?;
                let ack = r
                    .read_to_end(16)
                    .await
                    .map_err(|_| "Empfangsbestätigung fehlt")?;
                if ack != b"ok" {
                    return Err("Gegenstelle hat Umschlag abgewiesen");
                }
                let mut state = state.lock().expect("Netzzustand");
                state.activity.insert(c.stable_id(), Instant::now());
                if !state.allowed.contains_key(&o.to) {
                    return Err("Ziel nicht erlaubt");
                }
                Ok::<(), &str>(())
            })
            .await;
            match result {
                Ok(Ok(())) => {
                    reachable(&state, &out, &o.to, true);
                    let _ = out.send(NodeEvent::Delivered {
                        to: o.to.clone(),
                        tag: o.tag.clone(),
                    });
                    delivered = true;
                    break;
                }
                r => {
                    error = match r {
                        Ok(Err(e)) => e.into(),
                        _ => "Übertragung: Zeitlimit erreicht".into(),
                    };
                    if let Some(c) = used {
                        c.close(1u32.into(), b"retry");
                    }
                    reachable(&state, &out, &o.to, false);
                }
            }
            if attempt < 5 {
                if !state
                    .lock()
                    .expect("Netzzustand")
                    .allowed
                    .contains_key(&o.to)
                {
                    error = "Ziel nicht erlaubt".into();
                    break;
                }
                tokio::time::sleep(Duration::from_secs((1u64 << attempt).min(30))).await;
            }
        }
        if !delivered {
            failed(&out, &o, &error);
        }
        let _ = readers.try_join_next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Payload;
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(25)
    }
    fn event(n: &Node, until: Instant, wanted: impl Fn(&NodeEvent) -> bool) -> NodeEvent {
        loop {
            let e = n
                .events
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("Transport-Ereignis fehlt (25 s Gesamtlimit)");
            if wanted(&e) {
                return e;
            }
            if let NodeEvent::Error(ref error) = e {
                panic!("{error}");
            }
        }
    }
    fn address(n: &Node, until: Instant) -> String {
        let NodeEvent::Address(a) = event(n, until, |e| matches!(e, NodeEvent::Address(_))) else {
            panic!()
        };
        let addr: EndpointAddr = serde_json::from_str(&a).unwrap();
        let ips = addr.ip_addrs().map(|socket| {
            let ip = if socket.is_ipv4() {
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            } else {
                std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
            };
            iroh::TransportAddr::Ip(std::net::SocketAddr::new(ip, socket.port()))
        });
        // Tests ausschließlich über Loopback, nie über eine LAN-Schnittstelle routen.
        serde_json::to_string(&EndpointAddr::from_parts(addr.id, ips)).unwrap()
    }
    fn env(index: usize) -> Envelope {
        Envelope::new(Payload::Leave {
            chat: index.to_string().as_str().into(),
        })
    }
    fn outgoing(to: &PeerId, index: usize) -> Outgoing {
        Outgoing {
            to: to.clone(),
            env: env(index),
            tag: Some(index.to_string().as_str().into()),
        }
    }
    fn stop(n: &Node, until: Instant) {
        n.stop();
        event(n, until, |e| matches!(e, NodeEvent::Stopped));
    }
    #[test]
    fn loopback_optional_ausweis_exchange_keeps_tls_sender() {
        use crate::ausweis::{base64url, kanonisch, Ausweis, Inhalt, Rolle};
        let until = deadline();
        let identity = Identity::ephemeral();
        let peer = identity.peer();
        let issuer = iroh::SecretKey::from_bytes(&[81; 32]);
        let inhalt = Inhalt {
            v: 1,
            typ: "geraeteausweis".into(),
            pid: "transport-test".into(),
            person: "Aylin".into(),
            rolle: Rolle::Seminarleitung,
            net_pubkey: peer.to_string(),
            geraet_pubkey: base64url(&[82; 32]),
            iat: 1,
            exp: 43201,
        };
        let signatur = base64url(&issuer.sign(&kanonisch(&inhalt).unwrap()).to_bytes());
        let cert = Ausweis { inhalt, signatur };
        let a = Node::start_with_ausweis(identity, Some(cert.clone()));
        let b = Node::start(Identity::ephemeral());
        let aa = address(&a, until);
        let ba = address(&b, until);
        a.set_allowed(vec![(b.me.clone(), ba)]);
        b.set_allowed(vec![(a.me.clone(), aa)]);
        a.send(outgoing(&b.me, 0)).unwrap();
        let received = event(&b, until, |e| {
            matches!(
                e,
                NodeEvent::Received {
                    env: Envelope {
                        payload: Payload::Geraeteausweis { .. },
                        ..
                    },
                    ..
                }
            )
        });
        let NodeEvent::Received {
            from,
            env:
                Envelope {
                    payload: Payload::Geraeteausweis { ausweis },
                    ..
                },
        } = received
        else {
            panic!("Ausweis fehlt")
        };
        assert_eq!(from, peer);
        assert_eq!(ausweis, cert);
        assert_eq!(
            ausweis
                .pruefen(&base64url(issuer.public().as_bytes()), &from, 2)
                .unwrap()
                .rolle(),
            Rolle::Seminarleitung
        );
        stop(&a, until);
        stop(&b, until);
    }
    #[test]
    fn loopback_bidirectional_order_sender_delivered_and_stop() {
        let until = deadline();
        let a = Node::start(Identity::ephemeral());
        let b = Node::start(Identity::ephemeral());
        let aa = address(&a, until);
        let ba = address(&b, until);
        a.set_allowed(vec![(b.me.clone(), ba)]);
        b.set_allowed(vec![(a.me.clone(), aa)]);
        // Ein Auftrag auf demselben Befehlskanal hinter der Allowlist vermeidet Start-Rennen.
        a.send(outgoing(&b.me, 0)).unwrap();
        let e = event(&b, until, |e| matches!(e, NodeEvent::Received { .. }));
        assert_eq!(
            e,
            NodeEvent::Received {
                from: a.me.clone(),
                env: env(0)
            }
        );
        event(
            &a,
            until,
            |e| matches!(e, NodeEvent::Delivered { tag: Some(tag), .. } if tag.as_str() == "0"),
        );
        b.send(outgoing(&a.me, 99)).unwrap();
        assert_eq!(
            event(&a, until, |e| matches!(e, NodeEvent::Received { .. })),
            NodeEvent::Received {
                from: b.me.clone(),
                env: env(99)
            }
        );
        event(&b, until, |e| matches!(e, NodeEvent::Delivered { .. }));
        for index in 1..=20 {
            a.send(outgoing(&b.me, index)).unwrap();
        }
        for index in 1..=20 {
            assert_eq!(
                event(&b, until, |e| matches!(e, NodeEvent::Received { .. })),
                NodeEvent::Received {
                    from: a.me.clone(),
                    env: env(index)
                }
            );
            event(
                &a,
                until,
                |e| matches!(e, NodeEvent::Delivered { tag: Some(tag), .. } if tag.as_str() == index.to_string()),
            );
        }
        assert!(a
            .send(Outgoing {
                to: b.me.clone(),
                env: Envelope::new(Payload::RemoteInput {
                    req: "x".into(),
                    text: "x".repeat(MAX_ENVELOPE)
                }),
                tag: None
            })
            .is_err());
        stop(&a, until);
        stop(&b, until);
        assert!(a.send(outgoing(&b.me, 30)).is_err());
    }
    #[test]
    fn third_node_rejected_never_received() {
        let until = deadline();
        let a = Node::start(Identity::ephemeral());
        let b = Node::start(Identity::ephemeral());
        let c = Node::start(Identity::ephemeral());
        let aa = address(&a, until);
        let ba = address(&b, until);
        let _ca = address(&c, until);
        a.set_allowed(vec![(b.me.clone(), ba.clone())]);
        b.set_allowed(vec![(a.me.clone(), aa)]);
        // Erst eine erlaubte Zustellung als Schranke für die Kontaktliste am Empfänger.
        a.send(outgoing(&b.me, 0)).unwrap();
        event(&b, until, |e| matches!(e, NodeEvent::Received { .. }));
        c.set_allowed(vec![(b.me.clone(), ba)]);
        c.send(outgoing(&b.me, 1)).unwrap();
        assert!(
            matches!(event(&b, until, |e| matches!(e, NodeEvent::Rejected { .. })), NodeEvent::Rejected { from, .. } if from == c.me)
        );
        assert!(b
            .events
            .try_iter()
            .all(|e| !matches!(e, NodeEvent::Received { from, .. } if from == c.me)));
        stop(&c, until);
        stop(&a, until);
        stop(&b, until);
    }
    #[test]
    fn raw_stream_oversize_decode_rate_and_multiple_streams() {
        let until = deadline();
        let n = Node::start(Identity::ephemeral());
        let addr: EndpointAddr = serde_json::from_str(&address(&n, until)).unwrap();
        let identity = Identity::ephemeral();
        let peer = identity.peer();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(24), async {
                let ep = Endpoint::builder(presets::Minimal).secret_key(identity.secret).alpns(vec![ALPN.to_vec()]).bind_addr("127.0.0.1:0").unwrap().bind().await.unwrap();
                n.set_allowed(vec![(peer.clone(), serde_json::to_string(&ep.addr()).unwrap())]);
                let c = ep.connect(addr, ALPN).await.unwrap();
                async fn transfer(c: &Connection, data: &[u8]) -> Result<Vec<u8>, String> {
                    let (mut s, mut r) = c.open_bi().await.map_err(|e| e.to_string())?;
                    s.write_all(data).await.map_err(|e| e.to_string())?; s.finish().map_err(|e| e.to_string())?;
                    r.read_to_end(16).await.map_err(|e| e.to_string())
                }
                assert_eq!(transfer(&c, b"private-invalid-content").await.unwrap(), b"err");
                assert_eq!(n.events.recv_timeout(Duration::from_secs(1)).unwrap(), NodeEvent::Error("Umschlag ungültig oder zu groß".into()));
                // Zu großer Stream wird entweder negativ quittiert oder vom QUIC-Leselimit beendet.
                assert_ne!(transfer(&c, &vec![b'x'; MAX_ENVELOPE + 1]).await.unwrap_or_default(), b"ok");
                event(&n, until, |e| matches!(e, NodeEvent::Error(_)));
                for index in 0..(RATE_PER_MIN - 2) {
                    assert_eq!(transfer(&c, &env(index as usize).encode().unwrap()).await.unwrap(), b"ok");
                    assert_eq!(event(&n, until, |e| matches!(e, NodeEvent::Received { .. })), NodeEvent::Received { from: peer.clone(), env: env(index as usize) });
                }
                assert_ne!(transfer(&c, &env(999).encode().unwrap()).await.unwrap_or_default(), b"ok");
                assert!(matches!(event(&n, until, |e| matches!(e, NodeEvent::Rejected { .. })), NodeEvent::Rejected { from, .. } if from == peer));
                assert!(n.events.try_iter().all(|e| !matches!(e, NodeEvent::Received { .. })));
                ep.close().await;
            }).await.expect("24 s Gesamtlimit");
        });
        stop(&n, until);
    }
    #[test]
    fn stalled_stream_times_out_without_blocking_commands() {
        let until = deadline();
        let n = Node::start(Identity::ephemeral());
        let addr: EndpointAddr = serde_json::from_str(&address(&n, until)).unwrap();
        let identity = Identity::ephemeral();
        let peer = identity.peer();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(24), async {
                let ep = Endpoint::builder(presets::Minimal)
                    .secret_key(identity.secret)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .unwrap()
                    .bind()
                    .await
                    .unwrap();
                n.set_allowed(vec![(
                    peer.clone(),
                    serde_json::to_string(&ep.addr()).unwrap(),
                )]);
                let c = ep.connect(addr, ALPN).await.unwrap();
                let (mut s, _r) = c.open_bi().await.unwrap();
                s.write_all(b"{").await.unwrap(); // Kein FIN: absichtlich unvollständig.
                n.send(outgoing(&"nicht erlaubt".into(), 0)).unwrap();
                let mut saw_failed = false;
                let mut saw_timeout = false;
                while !saw_timeout {
                    match n.events.try_recv() {
                        Ok(NodeEvent::Failed { .. }) => saw_failed = true,
                        Ok(NodeEvent::Error(e)) => {
                            assert_eq!(e, "Stream: Zeitlimit erreicht");
                            saw_timeout = true;
                        }
                        Ok(_) => {}
                        Err(mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(Duration::from_millis(5)).await
                        }
                        Err(e) => panic!("{e}"),
                    }
                }
                assert!(saw_failed, "Steuerschleife war blockiert");
                c.closed().await;
                ep.close().await;
            })
            .await
            .expect("24 s Gesamtlimit");
        });
        stop(&n, until);
    }
    #[test]
    fn allowlist_revocation_closes_an_existing_tls_connection() {
        let until = deadline();
        let n = Node::start(Identity::ephemeral());
        let addr: EndpointAddr = serde_json::from_str(&address(&n, until)).unwrap();
        let identity = Identity::ephemeral();
        let peer = identity.peer();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(24), async {
                let ep = Endpoint::builder(presets::Minimal)
                    .secret_key(identity.secret)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .unwrap()
                    .bind()
                    .await
                    .unwrap();
                n.set_allowed(vec![(peer, serde_json::to_string(&ep.addr()).unwrap())]);
                let c = ep.connect(addr, ALPN).await.unwrap();
                let (mut s, mut r) = c.open_bi().await.unwrap();
                s.write_all(&env(1).encode().unwrap()).await.unwrap();
                s.finish().unwrap();
                assert_eq!(r.read_to_end(16).await.unwrap(), b"ok");
                assert!(matches!(
                    n.events.try_recv().unwrap(),
                    NodeEvent::Received { .. }
                ));
                n.set_allowed(vec![]);
                c.closed().await;
                assert!(c.close_reason().is_some());
                ep.close().await;
            })
            .await
            .expect("24 s Gesamtlimit");
        });
        stop(&n, until);
    }
    #[test]
    fn queue_is_globally_bounded_and_commands_nonblocking() {
        let (tx, _rx) = channel::channel(MAX_QUEUE + 2);
        let (out, events) = mpsc::channel();
        let n = Node {
            me: "me".into(),
            events,
            tx,
            out,
            queued: Arc::new(AtomicUsize::new(0)),
            stopping: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(Notify::new()),
        };
        for index in 0..MAX_QUEUE {
            n.send(outgoing(&"peer".into(), index)).unwrap();
        }
        assert_eq!(
            n.send(outgoing(&"peer".into(), 999)),
            Err("Warteschlange voll".into())
        );
        n.stop();
        assert!(n.stopping.load(Ordering::Acquire));
    }
    #[test]
    fn addresses_validate_id_and_revocation_closes_cached_connections() {
        let until = deadline();
        let a = Node::start(Identity::ephemeral());
        let b = Node::start(Identity::ephemeral());
        let aa = address(&a, until);
        let ba = address(&b, until);
        assert_eq!(parse_address(&ba).unwrap(), b.me);
        assert!(parse_address("invalid").is_err());
        a.set_allowed(vec![(a.me.clone(), ba.clone())]);
        event(&a, until, |e| matches!(e, NodeEvent::Error(_)));
        a.send(outgoing(&b.me, 0)).unwrap();
        event(&a, until, |e| matches!(e, NodeEvent::Failed { .. }));
        a.set_allowed(vec![(b.me.clone(), ba)]);
        b.set_allowed(vec![(a.me.clone(), aa)]);
        a.send(outgoing(&b.me, 1)).unwrap();
        event(&b, until, |e| matches!(e, NodeEvent::Received { .. }));
        event(&a, until, |e| matches!(e, NodeEvent::Delivered { .. }));
        a.set_allowed(vec![]);
        a.send(outgoing(&b.me, 2)).unwrap();
        event(&a, until, |e| matches!(e, NodeEvent::Failed { .. }));
        stop(&a, until);
        stop(&b, until);
    }
    #[test]
    fn loopback_retry_reconnects_and_preserves_queue_order() {
        let until = deadline();
        let n = Node::start(Identity::ephemeral());
        let _ = address(&n, until);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(24), async {
                let ep = Endpoint::builder(presets::Minimal)
                    .alpns(vec![ALPN.to_vec()])
                    .bind_addr("127.0.0.1:0")
                    .unwrap()
                    .bind()
                    .await
                    .unwrap();
                let peer = peer_of(&ep.id());
                n.set_allowed(vec![(
                    peer.clone(),
                    serde_json::to_string(&ep.addr()).unwrap(),
                )]);
                n.send(outgoing(&peer, 1)).unwrap();
                n.send(outgoing(&peer, 2)).unwrap();
                let first = ep.accept().await.unwrap().await.unwrap();
                let (mut s, mut r) = first.accept_bi().await.unwrap();
                assert_eq!(
                    Envelope::decode(&r.read_to_end(MAX_ENVELOPE).await.unwrap()).unwrap(),
                    env(1)
                );
                s.write_all(b"err").await.unwrap();
                s.finish().unwrap();
                first.closed().await;
                let second = ep.accept().await.unwrap().await.unwrap();
                for index in 1..=2 {
                    let (mut s, mut r) = second.accept_bi().await.unwrap();
                    assert_eq!(
                        Envelope::decode(&r.read_to_end(MAX_ENVELOPE).await.unwrap()).unwrap(),
                        env(index)
                    );
                    s.write_all(b"ok").await.unwrap();
                    s.finish().unwrap();
                }
                let mut states = Vec::new();
                let mut tags = Vec::new();
                while tags.len() < 2 {
                    let e = match n.events.try_recv() {
                        Ok(e) => e,
                        Err(mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                            continue;
                        }
                        Err(e) => panic!("Ereigniskanal: {e}"),
                    };
                    match e {
                        NodeEvent::Reachable { ok, .. } => states.push(ok),
                        NodeEvent::Delivered { tag, .. } => tags.push(tag.unwrap().0),
                        NodeEvent::Error(e) => panic!("{e}"),
                        _ => {}
                    }
                }
                assert_eq!(states, vec![false, true]);
                assert_eq!(tags, vec!["1", "2"]);
                ep.close().await;
            })
            .await
            .expect("24 s Gesamtlimit");
        });
        stop(&n, until);
    }
    #[test]
    fn drop_also_stops_and_closes_event_channel() {
        let until = deadline();
        let n = Node::start(Identity::ephemeral());
        let _ = address(&n, until);
        let Node { events, .. } = &n;
        assert!(events.try_recv().is_err());
        // Den Empfänger für die Beobachtung aus dem Objekt herauslösen, ohne den
        // öffentlichen Vertrag oder die Drop-Implementierung zu umgehen.
        let mut n = n;
        let (_, replacement) = mpsc::channel();
        let events = std::mem::replace(&mut n.events, replacement);
        drop(n);
        assert!(
            events
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .unwrap()
                == NodeEvent::Stopped
        );
    }
    #[test]
    fn sliding_rate_window_and_states_change_only_once() {
        let mut s = State::default();
        let now = Instant::now();
        let peer = "p".into();
        for _ in 0..RATE_PER_MIN {
            assert!(rate_allowed(&mut s, &peer, now));
        }
        assert!(!rate_allowed(&mut s, &peer, now + Duration::from_secs(59)));
        assert!(rate_allowed(&mut s, &peer, now + Duration::from_secs(60)));
        let state = Arc::new(Mutex::new(s));
        let (out, rx) = mpsc::channel();
        reachable(&state, &out, &peer, false);
        reachable(&state, &out, &peer, false);
        reachable(&state, &out, &peer, true);
        assert_eq!(rx.try_iter().count(), 2);
    }
}
