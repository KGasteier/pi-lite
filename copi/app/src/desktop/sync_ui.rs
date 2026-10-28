//! Eine Kartenfolge, keine zweite Anmeldung und kein eigener Netzwerktransport.
use super::*;
use checkin::Progress;

impl Desktop {
    /// Vom geprüften Loopback-Callback des Empfang-Adapters aufgerufen.
    /// Niemals Token/Code im UI darstellen oder daraus eine Person ableiten.
    pub fn auth_callback(&mut self, callback: AuthCallback) {
        if self.flow.stage == Stage::Browser && callback.attempt == self.flow.attempt {
            self.flow_call(Request::AuthCallback(callback));
        }
    }
    pub(super) fn flow_call(&mut self, request: Request) {
        self.flow_generation += 1;
        let generation = self.flow_generation;
        self.flow_pending = true;
        self.flow_failed = false;
        self.error = None;
        self.flow_retry = Some(request.clone());
        self.flow_poll = Some(Instant::now());
        if self.demo {
            // Szenen sind statisch. Kein Browser, keine Dateien, keine Worker.
            self.flow_pending = false;
            self.flow_failed = true;
            self.error = Some("Vorschau: Diese Karte zeigt erfundene Daten. Anmeldung und Dateiübertragung werden hier nicht ausgeführt.".into());
            return;
        }
        let (api, tx) = (self.api.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Reply::FlowCompleted {
                generation,
                reply: Box::new(api.call(request)),
            });
        });
    }
    pub(super) fn cancel_flow(&mut self) {
        if let Some(request) = self.flow.cancel() {
            self.flow_call(request);
        } else {
            self.flow_generation += 1;
            self.flow_pending = false;
            self.flow_failed = false;
            self.error = None;
        }
    }
    pub(super) fn flow_tick(&mut self, ctx: &egui::Context) {
        if !self.flow_pending
            && !self.flow_failed
            && !self.demo
            && self
                .flow_poll
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
        {
            if let Some(request) = self.flow.poll() {
                self.flow_call(request);
            }
        }
        if self.flow_pending || self.flow.poll().is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
    pub(super) fn flow_card(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(70.0);
            ui.vertical_centered(|ui| {
                egui::Frame::group(ui.style()).inner_margin(30.0).show(ui, |ui| {
                    ui.set_width(ui.available_width().min(820.0));
                    ui.set_max_width(820.0);
                    if let Some(notice) = &self.flow.notice { ui.label(notice); ui.add_space(14.0); }
                    match self.flow.stage.clone() {
                        Stage::Starting => {
                            ui.heading("Check-in wird vorbereitet …");
                            ui.label("Deine Anmeldung läuft im System-Browser. Hier startet kein Agent.");
                            self.cancel_button(ui);
                        }
                        Stage::Browser => {
                            ui.heading("Warte auf Anmeldung im Browser …");
                            ui.add_space(16.0);
                            ui.label("Melde dich dort mit deinem UdK-Konto an. Dieses Fenster wartet auf die geprüfte Bestätigung vom Empfang.");
                            if let Some(url) = &self.flow.browser_url {
                                if self.demo { ui.label("Vorschau · kein Browser wird geöffnet"); }
                                else if ui.small_button("Anmeldung im Browser öffnen").clicked() {
                                    // Der Adapter liefert ausschließlich die geprüfte OIDC-URL.
                                    if let Err(e) = pi_backend::cli::open_url(url) { self.error = Some(format!("Der Browser konnte nicht geöffnet werden. {e}")); }
                                }
                            }
                            self.cancel_button(ui);
                        }
                        Stage::Queue { position, .. } => {
                            ui.heading("Ein Arbeitsplatz wird für dich frei");
                            ui.add_space(18.0);
                            ui.label(format!("Alle 10 Arbeitsplätze sind belegt. Du bist Nr. {position} in der Warteschlange."));
                            ui.label("Du rückst automatisch nach. Lass dieses Fenster einfach offen.");
                            self.cancel_button(ui);
                        }
                        Stage::Pickup => {
                            ui.heading("Dateien vom letzten Mal abholen");
                            ui.add_space(16.0);
                            if let Some(pickup) = self.flow.receipt.as_ref().and_then(|r| r.pickup.as_ref()) {
                                ui.label(format!("Im verschlüsselten Abholfach liegen {}. Verfügbar bis {}.", bytes(pickup.bytes), pickup.expires));
                            }
                            ui.label("Sie werden in ~/cocopi zurückgespielt. Lokale Änderungen bleiben erhalten.");
                            ui.add_space(20.0);
                            if ui.button("Dateien vom letzten Mal abholen").clicked() {
                                if let Some(request) = self.flow.collect() { self.flow_call(request); }
                            }
                            if ui.small_button("Später abholen · weiter zur Auswahl").clicked() {
                                if let Some(request) = self.flow.plan() { self.flow_call(request); }
                            }
                            if ui.small_button("Check-in beenden …").clicked() { self.cancel_flow(); }
                        }
                        Stage::Planning => {
                            ui.heading("Dateien in ~/cocopi werden geprüft …");
                            ui.label("Es wird noch nichts übertragen.");
                            self.cancel_button(ui);
                        }
                        Stage::Selection => {
                            ui.heading("Was möchtest du nach R2 mitnehmen?");
                            ui.add_space(12.0);
                            ui.label("Aus ~/cocopi · nur deine Auswahl wird übertragen.");
                            ui.add_space(18.0);
                            for item in &mut self.flow.plan.items {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut item.selected, &item.name);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(bytes(item.bytes)); });
                                });
                                ui.add_space(8.0);
                            }
                            ui.add_space(12.0);
                            ui.label(format!("Builds/Cache werden nicht übertragen ({})", bytes(self.flow.plan.excluded_bytes)));
                            let total: u64 = self.flow.plan.items.iter().filter(|i| i.selected).map(|i| i.bytes).sum();
                            ui.add_space(22.0);
                            if ui.add_enabled(self.flow.plan.items.iter().any(|i| i.selected), egui::Button::new(format!("Auswahl senden und starten · {}", bytes(total)))).clicked() {
                                if let Some(request) = self.flow.upload() { self.flow_call(request); }
                            }
                            ui.horizontal(|ui| {
                                if ui.small_button("Ohne Dateien starten").clicked() {
                                    self.flow.without_files();
                                    self.apply(Reply::Flow(FlowReply::Waiting));
                                }
                                if ui.small_button("Check-in beenden …").clicked() { self.cancel_flow(); }
                            });
                        }
                        Stage::Transfer(kind) => {
                            ui.heading(match kind { TransferKind::Upload => "Deine Auswahl geht nach R2 …", TransferKind::Checkout => "Dateien kommen zurück …", TransferKind::Pickup => "Dateien vom letzten Mal kommen zurück …" });
                            ui.add_space(18.0);
                            if let Some(progress) = &self.flow.progress {
                                let fraction = if progress.total > 0 { progress.bytes as f32 / progress.total as f32 } else { 0.0 };
                                ui.add(egui::ProgressBar::new(fraction.clamp(0.0, 1.0)).text(format!("{} von {}", bytes(progress.bytes), bytes(progress.total))));
                            } else { ui.spinner(); ui.label("Übertragung wird vorbereitet …"); }
                            if kind == TransferKind::Checkout {
                                ui.add_space(14.0);
                                ui.label("Erst nach geprüfter Rückkehr der Dateien und bestätigter Leerung wird der Arbeitsplatz freigegeben.");
                            }
                            self.cancel_button(ui);
                        }
                        Stage::PreviewLoading => {
                            ui.heading("Check-out wird geprüft …");
                            ui.label("Noch wird nichts gelöscht oder übertragen.");
                            self.cancel_button(ui);
                        }
                        Stage::Preview => {
                            ui.heading("Deine Dateien kommen mit");
                            ui.add_space(20.0);
                            if let Some(preview) = &self.flow.preview {
                                ui.columns(2, |columns| {
                                    columns[0].label(RichText::new("Kommt zurück auf deinen Rechner").strong());
                                    columns[0].add_space(14.0);
                                    for file in &preview.personal {
                                        columns[0].label(&file.path);
                                        columns[0].label(RichText::new(format!("{} · {}", bytes(file.bytes), file.status)).small().color(QUIET));
                                        columns[0].add_space(10.0);
                                    }
                                    let total: u64 = preview.personal.iter().map(|f| f.bytes).sum();
                                    columns[0].label(format!("Zusammen {}", bytes(total)));
                                    columns[1].label(RichText::new("Wird gelöscht").strong());
                                    columns[1].add_space(14.0);
                                    columns[1].label(format!("Builds, Cache und temporäre Dateien · {}", bytes(preview.volatile_bytes)));
                                    columns[1].collapsing(format!("{} Dateien · Beispiele zeigen", preview.volatile_files), |ui| {
                                        for example in &preview.examples { ui.label(example); }
                                    });
                                    columns[1].add_space(12.0);
                                    columns[1].label("Auch alle übrigen Dateien auf R2 werden nach der sicheren Rückkehr entfernt. Unveränderte Dateien hast du schon lokal.");
                                });
                            }
                            ui.add_space(20.0);
                            let slot = self.flow.receipt.as_ref().map(|r| r.account.slot.as_str()).unwrap_or("der Arbeitsplatz");
                            ui.label(format!("Danach ist {slot} leer und für andere frei. Auf R2 bleibt nichts."));
                            ui.label("Hast du eine Datei lokal geändert, liegt die R2-Fassung daneben. Nichts wird überschrieben.");
                            ui.add_space(22.0);
                            if ui.button("Auschecken und Dateien holen").clicked() {
                                if let Some(request) = self.flow.checkout() { self.flow_call(request); }
                            }
                            if ui.small_button("Zurück zur Arbeit").clicked() { self.cancel_flow(); }
                        }
                        Stage::Cancelling => {
                            ui.heading("Abbruch wird bestätigt …");
                            ui.label("Bitte warte. Deine Dateien werden nicht aufgrund dieses Klicks gelöscht.");
                        }
                        Stage::Done => {
                            ui.heading("Alles zurück in ~/cocopi");
                            ui.add_space(18.0);
                            if let Some(summary) = &self.flow.summary {
                                ui.label(format!("{} Dateien · {} · Arbeitsplatz ausgecheckt und leer bestätigt.", summary.files, bytes(summary.bytes)));
                                if summary.conflicts > 0 { ui.add_space(12.0); ui.label(format!("{} Dateien hattest du lokal geändert – die R2-Fassung liegt daneben.", summary.conflicts)); }
                            }
                            ui.add_space(22.0);
                            if ui.button("Ordner öffnen").clicked() {
                                if self.demo { self.error = Some("Vorschau: Es wird kein echter Ordner geöffnet.".into()); }
                                else if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
                                    let path = std::path::PathBuf::from(home).join("cocopi");
                                    #[cfg(target_os = "linux")]
                                    let mut command = std::process::Command::new("xdg-open");
                                    #[cfg(target_os = "macos")]
                                    let mut command = std::process::Command::new("open");
                                    #[cfg(target_os = "windows")]
                                    let mut command = std::process::Command::new("explorer");
                                    if let Err(e) = command.arg(path).spawn() { self.error = Some(format!("Der Ordner konnte nicht geöffnet werden. {e}")); }
                                }
                            }
                            if ui.small_button("Zur Startkarte").clicked() { self.flow = Flow::default(); }
                        }
                        Stage::Start | Stage::Working => {}
                    }
                    if self.flow_failed {
                        ui.add_space(18.0);
                        if let Some(error) = &self.error { ui.label(error); }
                        // Mutierende Requests werden nicht blind wiederholt: Empfang/Sync
                        // muss dazu erst verbindliche Idempotenzschlüssel anbieten.
                        if matches!(self.flow_retry, Some(Request::Wait(_) | Request::SyncPlan(_) | Request::CheckoutPreview(_)))
                            && ui.button("Erneut prüfen").clicked()
                        {
                            if let Some(request) = self.flow_retry.clone() { self.flow_call(request); }
                        }
                    }
                });
            });
        });
    }
    fn cancel_button(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);
        if ui.small_button("Abbrechen").clicked() {
            self.cancel_flow();
        }
    }
    pub(super) fn flow_scene(&mut self, scene: &str) -> bool {
        let stage = match scene {
            "v3-checkin" => Stage::Browser,
            "v3-warteschlange" => Stage::Queue {
                id: "demo-queue".into(),
                position: 2,
            },
            "v3-sync-auswahl" => Stage::Selection,
            "v3-arbeiten-eingecheckt" | "v3-seminar-platzhalter" => Stage::Working,
            "v3-checkout-vorschau" => Stage::Preview,
            "v3-checkout-fertig" => Stage::Done,
            "v3-sync-fortschritt" => Stage::Transfer(TransferKind::Upload),
            "v3-checkout-fortschritt" => Stage::Transfer(TransferKind::Checkout),
            "v3-abholfach" => Stage::Pickup,
            _ => return false,
        };
        self.flow.stage = stage.clone();
        self.flow.attempt = 1;
        if !matches!(stage, Stage::Browser | Stage::Queue { .. } | Stage::Done) {
            self.flow.receipt = Some(MemoryApi::receipt());
            self.account = Some(MemoryApi::receipt().account);
            self.connected = true;
        }
        self.flow.plan = MemoryApi::sync_plan();
        self.flow.preview = Some(MemoryApi::checkout_preview());
        self.flow.summary = Some(MemoryApi::summary());
        self.flow.browser_url = Some("https://auth.example.invalid/cocopi-demo".into());
        self.flow.progress = if let Stage::Transfer(kind) = stage {
            Some(Progress {
                id: "demo-transfer".into(),
                kind,
                bytes: 1_100_000,
                total: 4_200_000,
            })
        } else {
            None
        };
        if stage == Stage::Pickup {
            self.flow.receipt.as_mut().unwrap().pickup = Some(checkin::Pickup {
                id: "demo-pickup".into(),
                bytes: 2_200_000,
                expires: "morgen, 23:30".into(),
            });
        }
        if stage == Stage::Working {
            self.welcome_done = true;
            self.seminar_selected = scene == "v3-seminar-platzhalter";
            self.sessions = vec![Session {
                id: "a".into(),
                title: "Seminar-Handout".into(),
                state: SessionState::Working,
                local: false,
                pinned: true,
            }];
            self.selected = Some("a".into());
            self.messages = vec![ChatMessage::new("Mira", "Mach aus meiner Gliederung ein zweiseitiges Seminar-Handout. Noch nichts veröffentlichen."), ChatMessage::new("cocopi · R2", "Ich arbeite mit deiner freigegebenen Auswahl. Das Ergebnis bleibt auf R2, bis du es abholst.")];
        }
        true
    }
}
fn bytes(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1} GB", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1} MB", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0} KB", n as f64 / 1_000.0)
    } else {
        format!("{n} B")
    }
}
