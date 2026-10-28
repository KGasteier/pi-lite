//! Einbettung: `#[cfg(feature="cocopi")] pub mod seminar;` in desktop.rs.
//! Genau ein Tab ruft `Seminar::ui` auf. Runtime über SeminarNet austauschbar.
use egui::{Color32, RichText, Stroke};
#[path = "seminar_net.rs"]
mod seminar_net;
pub use pi_net::seminar::{HubPrompt, MemoryNet, SeminarNet, SeminarState};
use pi_net::{
    model::{ChatKind, PeerId, now_ms},
    remote::{GrantState, RemoteScope, Side},
    seminar::Effects,
    store::Outbox,
};
pub use seminar_net::production;
const CYAN: Color32 = Color32::from_rgb(0x4f, 0xb8, 0xd8);

pub struct Seminar {
    pub state: SeminarState,
    net: Box<dyn SeminarNet>,
    input: String,
    control_input: String,
    session_indices: std::collections::BTreeMap<PeerId, usize>,
    focused_request: Option<String>,
    pub error: Option<String>,
    pub marked_prompts: Vec<HubPrompt>,
    demo_now: Option<u64>,
}
impl Seminar {
    pub fn new(state: SeminarState, net: Box<dyn SeminarNet>) -> Self {
        Self {
            state,
            net,
            input: String::new(),
            control_input: String::new(),
            session_indices: Default::default(),
            focused_request: None,
            error: None,
            marked_prompts: vec![],
            demo_now: None,
        }
    }
    fn now(&self) -> u64 {
        self.demo_now.unwrap_or_else(now_ms)
    }
    fn dispatch(&mut self, out: Result<Outbox, String>) {
        match out {
            Ok(out) => {
                for (to, p) in out {
                    if let Err(e) = self.net.send(&to, p) {
                        self.error = Some(format!("Nicht gesendet: {e}"));
                    }
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn effects(&mut self, e: Effects) {
        self.dispatch(Ok(e.out));
        for h in e.hub {
            match self.net.senden(&h.session, &h.text, &h.von) {
                Ok(()) => {
                    self.marked_prompts.push(h);
                    if self.marked_prompts.len() > 100 {
                        self.marked_prompts.remove(0);
                    }
                }
                Err(e) => self.error = Some(format!("Auftrag nicht an R2 gesendet: {e}")),
            }
        }
    }
    /// TLS-authentifizierte Ingress-Events und Presence muss der Runtime-Adapter liefern.
    pub fn receive(&mut self, from: &PeerId, p: pi_net::wire::Payload) {
        let now = self.now();
        match self.state.receive(from, p, now) {
            Ok(e) => self.effects(e),
            Err(e) => self.error = Some(e),
        }
    }
    pub fn set_contacts(&mut self, peers: Vec<(PeerId, String)>) -> Result<(), String> {
        if peers
            .iter()
            .any(|(peer, _)| !self.state.people.iter().any(|p| &p.peer == peer))
        {
            return Err("Kontakt fehlt im bestätigten Seminarverzeichnis".into());
        }
        self.net.set_contacts(peers)
    }
    pub fn checkout(&mut self) {
        let out = self.state.checkout();
        self.dispatch(Ok(out));
        self.net.checkout();
        self.marked_prompts.clear();
        self.input.clear();
        self.control_input.clear();
    }
    /// Rahmen/Status muss der Desktop zusätzlich über dem Hauptchat sichtbar halten,
    /// wenn Seminar nicht der aktive Tab ist. Kein Zugriff auf lokale Prozesse.
    fn paint_border(&self, ui: &egui::Ui) {
        if self.state.remote.grants.iter().any(
            |g| matches!(g.state, GrantState::Active { expires_ms } if expires_ms > self.now()),
        ) {
            ui.painter().rect_stroke(
                ui.max_rect().shrink(1.0),
                0.0,
                Stroke::new(2.0, CYAN),
                egui::StrokeKind::Inside,
            );
        }
    }
    /// Immer aufrufen, auch wenn der Seminar-Tab geschlossen ist.
    pub fn tick(&mut self) {
        let now = self.now();
        let incoming = self.net.poll();
        for (peer, online, since) in self.net.presence() {
            self.state.status(&peer, online, since);
        }
        for error in self.net.errors() {
            self.error = Some(error);
        }
        for (from, p) in incoming {
            match self.state.receive(&from, p, now) {
                Ok(e) => self.effects(e),
                Err(e) => self.error = Some(e),
            }
        }
        let out = self.state.tick(now);
        self.dispatch(Ok(out));
    }
    pub fn remote_bar(&mut self, ui: &mut egui::Ui) {
        self.tick();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
        self.paint_border(ui);
        let now = self.now();
        let grants: Vec<_> = self
            .state
            .remote
            .grants
            .iter()
            .filter(|g| matches!(g.state,GrantState::Active{expires_ms} if expires_ms>now))
            .cloned()
            .collect();
        for g in grants {
            let GrantState::Active { expires_ms } = g.state else {
                continue;
            };
            let person = self.state.name(&g.peer, now);
            let label = g
                .session
                .as_ref()
                .map(|s| s.label.as_str())
                .unwrap_or("Session");
            let control = g.scope == RemoteScope::Control;
            let text = match (g.side, control) {
                (Side::Host, true) => format!("{person} steuert mit"),
                (Side::Host, false) => format!("{person} sieht zu"),
                (Side::Viewer, true) => format!("Du steuerst {person}s Session ‚{label}‘"),
                (Side::Viewer, false) => format!("Du siehst {person}s Session ‚{label}‘"),
            };
            egui::Frame::new()
                .fill(Color32::from_rgb(24, 49, 59))
                .stroke(Stroke::new(1.0, CYAN))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(text).color(CYAN).strong());
                        ui.label(format!(
                            "· noch {} min",
                            (expires_ms - now).div_ceil(60_000)
                        ));
                        if ui
                            .button(if g.side == Side::Host {
                                "Beenden"
                            } else {
                                "Zurückgeben"
                            })
                            .clicked()
                        {
                            let out = self.state.end(&g.req, "Freigabe beendet");
                            self.dispatch(Ok(out));
                        }
                    });
                });
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.tick();
        let now = self.now();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
        let active = self
            .state
            .remote
            .grants
            .iter()
            .any(|g| matches!(g.state, GrantState::Active { .. }));
        let frame = egui::Frame::new()
            .inner_margin(18.0)
            .stroke(Stroke::new(if active { 2.0 } else { 0.0 }, CYAN));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Seminar");
                ui.label(
                    RichText::new("Direkt miteinander · nur mit Zustimmung mitsteuern").weak(),
                );
            });
            ui.add_space(16.0);
            self.remote_bar(ui);
            if let Some(e) = &self.error {
                ui.colored_label(Color32::from_rgb(240, 171, 126), e);
            }
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(280.0, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.people_ui(ui, now),
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.chat_ui(ui, now),
                );
            });
        });
        self.consent_ui(ui);
    }
    fn people_ui(&mut self, ui: &mut egui::Ui, now: u64) {
        ui.label(RichText::new("Teilnehmende").strong());
        ui.add_space(10.0);
        let people = self.state.people.clone();
        for p in people {
            egui::Frame::new().inner_margin(10.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if p.online { CYAN } else { Color32::GRAY },
                        if p.online { "•" } else { "–" },
                    );
                    ui.label(RichText::new(self.state.name(&p.peer, now)).strong());
                });
                ui.label(RichText::new(self.state.badge(&p.peer, now)).color(
                    if self.state.verified(&p.peer, now).is_some() {
                        CYAN
                    } else {
                        Color32::GRAY
                    },
                ));
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        if p.online { "online" } else { "offline" },
                        p.slot.as_deref().unwrap_or("nicht eingecheckt")
                    ))
                    .small()
                    .weak(),
                );
                if ui.button("Nachricht").clicked() {
                    let out = self.state.direct(&p.peer, now);
                    self.dispatch(out);
                }
                if self.state.can_request(&p.peer, now) {
                    let mut index = *self.session_indices.get(&p.peer).unwrap_or(&0);
                    egui::ComboBox::from_id_salt(("session", p.peer.as_str()))
                        .selected_text(
                            p.sessions
                                .get(index)
                                .map(|s| s.label.as_str())
                                .unwrap_or("Keine Session"),
                        )
                        .show_ui(ui, |ui| {
                            for (i, s) in p.sessions.iter().enumerate() {
                                ui.selectable_value(&mut index, i, &s.label);
                            }
                        });
                    self.session_indices.insert(p.peer.clone(), index);
                    let existing = self
                        .state
                        .remote
                        .grants
                        .iter()
                        .find(|g| {
                            g.peer == p.peer
                                && g.side == Side::Viewer
                                && !matches!(g.state, GrantState::Ended { .. })
                        })
                        .cloned();
                    if let Some(g) = existing {
                        if g.state == GrantState::Requested {
                            ui.label("Anfrage gesendet · wartet auf Zustimmung");
                            if ui.button("Anfrage zurückziehen").clicked() {
                                let out = self.state.end(&g.req, "Anfrage zurückgezogen");
                                self.dispatch(Ok(out));
                            }
                        }
                    } else if let Some(session) = p.sessions.get(index) {
                        if ui.button("Ansehen").clicked() {
                            let out = self.state.request(&p.peer, session, RemoteScope::View, now);
                            self.dispatch(out);
                        }
                        if ui.button("Mitsteuern anfragen").clicked() {
                            let out =
                                self.state
                                    .request(&p.peer, session, RemoteScope::Control, now);
                            self.dispatch(out);
                        }
                    }
                }
            });
            ui.separator();
        }
    }
    fn chat_ui(&mut self, ui: &mut egui::Ui, now: u64) {
        if self.state.group.is_none()
            && self
                .state
                .verified(&self.state.chats.me, now)
                .is_some_and(|c| c.rolle() == pi_net::ausweis::Rolle::Seminarleitung)
            && ui.button("Seminarchat öffnen").clicked()
        {
            let out = self.state.create_group(now);
            self.dispatch(out);
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(group) = &self.state.group {
                if ui
                    .selectable_label(self.state.selected.as_ref() == Some(group), "Seminarchat")
                    .clicked()
                {
                    self.state.selected = Some(group.clone());
                }
            }
            let dms: Vec<_> = self
                .state
                .chats
                .chats
                .iter()
                .filter(|c| c.kind == ChatKind::Direct)
                .map(|c| (c.id.clone(), c.title.clone()))
                .collect();
            for (id, title) in dms {
                if ui
                    .selectable_label(
                        self.state.selected.as_ref() == Some(&id),
                        format!("Nachricht · {title}"),
                    )
                    .clicked()
                {
                    self.state.selected = Some(id);
                }
            }
        });
        ui.add_space(12.0);
        let invites = self.state.chats.invites.clone();
        for invite in invites {
            ui.horizontal(|ui| {
                ui.label(format!("Einladung: {}", invite.chat.title));
                if ui.button("Annehmen").clicked() {
                    let id = invite.chat.id.clone();
                    let group = invite.chat.kind == ChatKind::Group;
                    let out = self.state.chats.accept_invite(&id);
                    if out.is_ok() {
                        self.state.selected = Some(id.clone());
                        if group {
                            self.state.group = Some(id);
                        }
                    }
                    self.dispatch(out);
                }
                if ui.button("Ablehnen").clicked() {
                    let out = self.state.chats.decline_invite(&invite.chat.id);
                    self.dispatch(Ok(out));
                }
            });
        }
        let messages = self
            .state
            .selected
            .as_ref()
            .and_then(|id| self.state.chats.chat(id))
            .map(|c| c.messages.clone())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .id_salt("seminar-messages")
            .max_height((ui.available_height() - 180.0).max(140.0))
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for m in messages {
                    let peer = m.author.device();
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            self.state.name(peer, now),
                            self.state.badge(peer, now)
                        ))
                        .color(CYAN)
                        .strong(),
                    );
                    ui.label(m.body.text());
                    ui.label(
                        RichText::new(match m.delivery {
                            pi_net::model::Delivery::Pending => "wird gesendet",
                            pi_net::model::Delivery::Failed(_) => "nicht gesendet",
                            _ => "",
                        })
                        .small()
                        .weak(),
                    );
                    ui.add_space(16.0);
                }
                for h in &self.marked_prompts {
                    ui.label(RichText::new(h.von.label()).color(CYAN).strong());
                    ui.label(&h.text);
                }
            });
        let viewer_grants: Vec<_> = self
            .state
            .remote
            .grants
            .iter()
            .filter(|g| {
                g.side == Side::Viewer
                    && matches!(g.state,GrantState::Active{expires_ms} if expires_ms>now)
            })
            .cloned()
            .collect();
        for grant in viewer_grants {
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Freigegebene Session · {}",
                    grant
                        .session
                        .as_ref()
                        .map(|s| s.label.as_str())
                        .unwrap_or("Session")
                ))
                .color(CYAN),
            );
            if grant.mirror.is_empty() {
                ui.label("Wartet auf freigegebenen Verlauf …");
            }
            for item in &grant.mirror {
                let text = match item {
                    pi_net::remote::MirrorItem::User { text }
                    | pi_net::remote::MirrorItem::Assistant { text, .. }
                    | pi_net::remote::MirrorItem::Note { text } => text.clone(),
                    pi_net::remote::MirrorItem::Tool { name, detail, .. } => {
                        format!("{name} · {detail}")
                    }
                };
                ui.label(text);
            }
        }
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .hint_text("Nachricht schreiben …")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        if ui
            .add_enabled(
                !self.input.trim().is_empty() && self.state.selected.is_some(),
                egui::Button::new("Nachricht senden"),
            )
            .clicked()
        {
            let text = std::mem::take(&mut self.input);
            let out = self.state.post(&text);
            if out.is_err() {
                self.input = text;
            }
            self.dispatch(out);
        }
        let viewer = self
            .state
            .remote
            .grants
            .iter()
            .find(|g| {
                g.side == Side::Viewer
                    && g.scope == RemoteScope::Control
                    && matches!(g.state,GrantState::Active{expires_ms} if expires_ms>now)
            })
            .cloned();
        if let Some(g) = viewer {
            ui.separator();
            ui.colored_label(CYAN, "Auftrag für die freigegebene Session");
            ui.add(
                egui::TextEdit::multiline(&mut self.control_input)
                    .hint_text("Als Seminarleitung senden …")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            if ui
                .add_enabled(
                    !self.control_input.trim().is_empty(),
                    egui::Button::new("Als Seminarleitung senden"),
                )
                .clicked()
            {
                let text = std::mem::take(&mut self.control_input);
                let out = self.state.control(&g.req, &text, now);
                if out.is_err() {
                    self.control_input = text;
                }
                self.dispatch(out);
            }
        }
    }
    /// Auf äußerstem Desktop-UI aufrufen, auch bei geschlossenem Seminar-Tab.
    pub fn consent_ui(&mut self, ui: &mut egui::Ui) {
        let now = self.now();
        let request = self
            .state
            .remote
            .grants
            .iter()
            .find(|g| g.side == Side::Host && g.state == GrantState::Requested)
            .cloned();
        let Some(g) = request else { return };
        let name = self.state.name(&g.peer, now);
        let session = self
            .state
            .sessions
            .iter()
            .find(|s| s.label == g.reason)
            .cloned();
        let mut choice = None;
        egui::Modal::new(egui::Id::new(("seminar-consent", &g.req))).show(ui.ctx(), |ui| {
            ui.set_width(470.0);
            ui.heading("Mitschreiben erlauben?");
            ui.add_space(12.0);
            ui.label(format!(
                "{name} (Seminarleitung) möchte in ‚{}‘ {}.",
                g.reason,
                if g.scope == RemoteScope::Control {
                    "mitschreiben"
                } else {
                    "zusehen"
                }
            ));
            ui.label(
                "Du kannst die Freigabe jederzeit beenden. Rückfragen beantwortest weiterhin du.",
            );
            ui.add_space(18.0);
            if session.is_none() {
                ui.label("Diese Session ist nicht mehr verfügbar. Lehne die Anfrage ab.");
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        session.is_some() && g.scope == RemoteScope::Control,
                        egui::Button::new("Erlauben 30 min"),
                    )
                    .clicked()
                {
                    choice = Some(Some(RemoteScope::Control));
                }
                if ui
                    .add_enabled(session.is_some(), egui::Button::new("Nur ansehen"))
                    .clicked()
                {
                    choice = Some(Some(RemoteScope::View));
                }
                let reject = ui.button("Ablehnen");
                if self.focused_request.as_ref() != Some(&g.req) {
                    reject.request_focus();
                    self.focused_request = Some(g.req.clone());
                }
                if reject.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    choice = Some(None);
                }
            });
        });
        if let Some(choice) = choice {
            let out = match (choice, session) {
                (Some(scope), Some(session)) => self.state.accept(&g.req, scope, &session, now),
                _ => Ok(self.state.end(&g.req, "Abgelehnt")),
            };
            self.dispatch(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(time: f64) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            time: Some(time),
            focused: true,
            ..Default::default()
        }
    }
    #[test]
    fn all_scenes_render_and_unknown_declines() {
        assert!(scene("unknown").is_none());
        for name in [
            "v3-seminar-chat",
            "v3-seminar-dm",
            "v3-mitsteuern-anfrage",
            "v3-mitsteuern-aktiv-student",
            "v3-mitsteuern-aktiv-leitung",
        ] {
            let mut seminar = scene(name).expect(name);
            let ctx = egui::Context::default();
            for n in 0..3 {
                let mut out = ctx.run_ui(input(n as f64 / 10.0), |ui| seminar.ui(ui));
                assert!(!out.shapes.is_empty());
                out.textures_delta.clear();
            }
            assert!(seminar.error.is_none(), "{name}: {:?}", seminar.error);
        }
    }
    #[test]
    fn reject_has_focus_and_enter_never_grants_control() {
        let mut seminar = scene("v3-mitsteuern-anfrage").unwrap();
        let ctx = egui::Context::default();
        for n in 0..3 {
            let mut out = ctx.run_ui(input(n as f64 / 10.0), |ui| seminar.ui(ui));
            out.textures_delta.clear();
        }
        assert!(ctx.memory(|m| m.focused()).is_some());
        let mut raw = input(0.4);
        raw.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut out = ctx.run_ui(raw, |ui| seminar.ui(ui));
        out.textures_delta.clear();
        assert!(
            seminar
                .state
                .remote
                .grants
                .iter()
                .all(|g| !matches!(g.state, GrantState::Active { .. })),
            "Enter darf nicht freigeben"
        );
        assert!(
            seminar.state.remote.grants.is_empty(),
            "Fokussiertes Ablehnen muss die Anfrage entfernen"
        );
    }
}

/// Eigene Demo-Szenen; Desktop::scene delegiert bei der Integration hierher.
pub fn scene(name: &str) -> Option<Seminar> {
    if ![
        "v3-seminar-chat",
        "v3-seminar-dm",
        "v3-mitsteuern-anfrage",
        "v3-mitsteuern-aktiv-student",
        "v3-mitsteuern-aktiv-leitung",
    ]
    .contains(&name)
    {
        return None;
    }
    let now = 1_000_000;
    let (mut a, mut b) = pi_net::seminar::demo_pair(now - 180_000);
    let group = a.create_group(now).ok()?;
    for (to, p) in group {
        if to == b.chats.me {
            b.receive(&a.chats.me, p, now).ok()?;
        }
    }
    let id = b.chats.invites.first()?.chat.id.clone();
    b.chats.accept_invite(&id).ok()?;
    b.group = Some(id.clone());
    b.selected = Some(id);
    for (to, p) in a
        .post("Willkommen! Heute gestalten wir ein zweiseitiges Seminar-Handout.")
        .ok()?
    {
        if to == b.chats.me {
            b.receive(&a.chats.me, p, now).ok()?;
        }
    }
    for (_, p) in b
        .post("Meine Gliederung steht. Kannst du kurz auf die Einleitung schauen?")
        .ok()?
    {
        a.receive(&b.chats.me, p, now).ok()?;
    }
    if name == "v3-seminar-dm" {
        let out = a.direct(&b.chats.me, now).ok()?;
        for (_, p) in out {
            b.receive(&a.chats.me, p, now).ok()?;
        }
        let id = b.chats.invites.first()?.chat.id.clone();
        b.chats.accept_invite(&id).ok()?;
        b.selected = Some(id);
        for (_, p) in a
            .post("Gern. Welche Stelle möchtest du gemeinsam überarbeiten?")
            .ok()?
        {
            b.receive(&a.chats.me, p, now).ok()?;
        }
    }
    let mut marked = vec![];
    if name.starts_with("v3-mitsteuern") {
        let session = b.sessions[0].clone();
        let (_, p) = a
            .request(&b.chats.me, &session, RemoteScope::Control, now)
            .ok()?
            .remove(0);
        b.receive(&a.chats.me, p, now).ok()?;
        if name != "v3-mitsteuern-anfrage" {
            let req = b.remote.grants[0].req.clone();
            let (_, p) = b
                .accept(&req, RemoteScope::Control, &session, now - 180_000)
                .ok()?
                .remove(0);
            a.receive(&b.chats.me, p, now).ok()?;
            let (_, p) = a
                .control(&req, "Kürze die Einleitung auf drei klare Sätze.", now)
                .ok()?
                .remove(0);
            let (_, frame)=b.mirror(&req,&session,pi_net::remote::RemoteFrame {seq:1,reset:true,items:vec![pi_net::remote::MirrorItem::Assistant{text:"Einleitung: Wir erkunden, wie Gestaltung verständlich wird. Das Handout bündelt drei Beobachtungen und eine gemeinsame Übung.".into(),done:true}],state:"ruht".into()},now).ok()?.remove(0);
            a.receive(&b.chats.me, frame, now).ok()?;
            marked = b.receive(&a.chats.me, p, now).ok()?.hub;
        }
    }
    let mut s = Seminar::new(
        if name.ends_with("leitung") { a } else { b },
        Box::<MemoryNet>::default(),
    );
    s.demo_now = Some(now);
    s.marked_prompts = marked;
    Some(s)
}
