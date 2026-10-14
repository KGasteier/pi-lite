//! Experiment: flüchtige, manuell gepaarte Direktverbindung; noch KEIN MLS-Gruppennetz.
use serde::{Deserialize, Serialize};
use std::{sync::mpsc, time::Duration};
use iroh::{Endpoint, EndpointAddr, endpoint::presets};
const ALPN: &[u8] = b"cocopi/direct-prototype/1";
pub const MAX_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Kind { Chat, Code, Prompt }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card { pub kind: Kind, pub text: String }
impl Card {
    pub fn new(kind: Kind, text: &str) -> Result<Self,String> {
        let text: String=text.chars().filter(|c| !c.is_control() || *c=='\n' || *c=='\t').filter(|c| !matches!(*c,'\u{200b}'..='\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2060}'..='\u{206f}'|'\u{feff}')).collect();
        if text.trim().is_empty() || text.len()>MAX_BYTES { return Err("Inhalt leer oder größer als 32 KiB".into()); }
        Ok(Self{kind,text})
    }
}
#[derive(Debug)]
pub enum Event { Address(String), Connected(String), Received(Card), Sent, Error(String), Stopped }
enum Command { Pair(String), Send(Card), Stop }
pub struct Node { tx: tokio::sync::mpsc::Sender<Command>, pub events: mpsc::Receiver<Event> }
impl Node {
    /// Start nur nach ausdrücklichem Klick. Minimal: keine öffentlichen Relays/Discovery.
    pub fn start() -> Self {
        let (tx,mut rx)=tokio::sync::mpsc::channel(16); let (out,events)=mpsc::channel();
        std::thread::spawn(move || {
            let rt=tokio::runtime::Runtime::new().unwrap();
            let failures=out.clone();
            let result:Result<(),String>=rt.block_on(async move {
                let endpoint=Endpoint::builder(presets::Minimal).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e|e.to_string())?;
                let address=serde_json::to_string(&endpoint.addr()).map_err(|e|e.to_string())?;
                let _=out.send(Event::Address(address));
                let mut peer:Option<EndpointAddr>=None;
                loop { tokio::select! {
                    cmd=rx.recv()=>match cmd {
                        Some(Command::Pair(ticket))=>{
                            match serde_json::from_str::<EndpointAddr>(&ticket) {
                                Ok(addr)=>{ let _=out.send(Event::Connected(addr.id.to_string())); peer=Some(addr); }
                                Err(e)=>{let _=out.send(Event::Error(format!("Kontaktadresse ungültig: {e}")));}
                            }
                        }
                        Some(Command::Send(card))=>{
                            let result=async {
                                let addr=peer.clone().ok_or("Kein Kontakt bestätigt")?;
                                let c=endpoint.connect(addr,ALPN).await.map_err(|e|e.to_string())?;
                                let (mut s,mut r)=c.open_bi().await.map_err(|e|e.to_string())?;
                                let data=serde_json::to_vec(&card).map_err(|e|e.to_string())?;
                                s.write_all(&data).await.map_err(|e|e.to_string())?; s.finish().map_err(|e|e.to_string())?;
                                let ack=r.read_to_end(16).await.map_err(|e|e.to_string())?;
                                if ack!=b"ok" {return Err("Keine Empfangsbestätigung".into());}
                                c.close(0u32.into(),b"done"); Ok::<_,String>(())
                            };
                            match tokio::time::timeout(Duration::from_secs(10),result).await {
                                Ok(Ok(()))=>{let _=out.send(Event::Sent);}
                                Ok(Err(e))=>{let _=out.send(Event::Error(e));}
                                Err(_)=>{let _=out.send(Event::Error("Verbindung: Zeitlimit erreicht".into()));}
                            }
                        }
                        Some(Command::Stop)|None=>break,
                    },
                    incoming=endpoint.accept()=>{
                        if let Some(incoming)=incoming {
                            let expected=peer.as_ref().map(|p|p.id);
                            let result=async {
                                let c=incoming.await.map_err(|e|e.to_string())?;
                                if Some(c.remote_id())!=expected {c.close(1u32.into(),b"unpaired");return Err("Unbestätigter Kontakt abgewiesen".into());}
                                let (mut s,mut r)=c.accept_bi().await.map_err(|e|e.to_string())?;
                                let data=r.read_to_end(MAX_BYTES+1024).await.map_err(|e|e.to_string())?;
                                let card:Card=serde_json::from_slice(&data).map_err(|e|e.to_string())?;
                                let card=Card::new(card.kind,&card.text)?;
                                out.send(Event::Received(card)).map_err(|e|e.to_string())?;
                                s.write_all(b"ok").await.map_err(|e|e.to_string())?; s.finish().map_err(|e|e.to_string())?;
                                c.closed().await; Ok::<_,String>(())
                            };
                            if let Ok(Err(e))=tokio::time::timeout(Duration::from_secs(10),result).await {let _=out.send(Event::Error(e));}
                        }
                    }
                }}
                endpoint.close().await; let _=out.send(Event::Stopped); Ok(())
            });
            if let Err(e)=result { let _=failures.send(Event::Error(e)); }
        });
        Self{tx,events}
    }
    pub fn pair(&self, address:String)->Result<(),String>{self.tx.try_send(Command::Pair(address)).map_err(|e|e.to_string())}
    pub fn send(&self,card:Card)->Result<(),String>{self.tx.try_send(Command::Send(card)).map_err(|e|e.to_string())}
}
impl Drop for Node { fn drop(&mut self){let _=self.tx.try_send(Command::Stop);} }
#[derive(Clone,Copy,PartialEq,Serialize,Deserialize)]
pub enum SpaceKind { Project, Seminar }
#[derive(Serialize,Deserialize)]
pub struct Space {pub name:String,pub kind:SpaceKind,pub hand:bool,pub step:usize,pub follow:bool}
impl Space {
    pub fn create(name:&str,kind:SpaceKind,verified_teacher:bool)->Result<Self,String>{
        if name.trim().is_empty(){return Err("Name fehlt".into());}
        if kind==SpaceKind::Seminar&&!verified_teacher{return Err("Seminar benötigt geprüfte Lehrendenrolle; authentik noch nicht angebunden".into());}
        Ok(Self{name:name.trim().into(),kind,hand:false,step:1,follow:false})
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn consent_and_roles(){assert!(Space::create("Seminar",SpaceKind::Seminar,false).is_err());assert!(Space::create("Projekt",SpaceKind::Project,false).is_ok());}
    #[test]
    fn two_real_lan_nodes_transfer_prompt_without_agent() {
        fn await_event(n:&Node, predicate:impl Fn(&Event)->bool)->Event {
            let deadline=std::time::Instant::now()+Duration::from_secs(20);
            loop { let e=n.events.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())).expect("Transport-Ereignis fehlt");
                if let Event::Error(ref s)=e {panic!("{s}");} if predicate(&e){return e;}
            }
        }
        let a=Node::start();let b=Node::start();
        let Event::Address(aa)=await_event(&a,|e|matches!(e,Event::Address(_))) else {unreachable!()};
        let Event::Address(bb)=await_event(&b,|e|matches!(e,Event::Address(_))) else {unreachable!()};
        a.pair(bb).unwrap();b.pair(aa).unwrap();
        await_event(&a,|e|matches!(e,Event::Connected(_)));await_event(&b,|e|matches!(e,Event::Connected(_)));
        a.send(Card::new(Kind::Prompt,"Synthetischer Testprompt").unwrap()).unwrap();
        let Event::Received(c)=await_event(&b,|e|matches!(e,Event::Received(_))) else {unreachable!()};
        assert_eq!(c.text,"Synthetischer Testprompt");await_event(&a,|e|matches!(e,Event::Sent));
    }
    #[test] fn hostile_prompt_is_data(){let c=Card::new(Kind::Prompt,"Hallo\u{202e}Welt\0").unwrap();assert_eq!(c.text,"HalloWelt");assert!(Card::new(Kind::Code,&"x".repeat(MAX_BYTES+1)).is_err());}
}
