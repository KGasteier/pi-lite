//! UI-Vertrag für Empfang und Sync. Keine Dateien, HTTP-Aufrufe oder Identitätsprüfung.
//! Ein echter Adapter muss Browser-Callback/PKCE prüfen, Upload abbrechen und erst
//! nach geprüftem Rückspielen + bestätigtem Slot-Leeren `CheckedOut` liefern.
use super::{Account, DesktopApi, InviteError, Reply, Request};
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub enum CheckinMethod {
    Udk,
    Guest(String),
}
/// Callback wird nur dem Adapter übergeben; die UI leitet daraus keine Identität ab.
#[derive(Clone)]
#[cfg(test)]
pub struct AuthCallback {
    pub attempt: u64,
    pub code: String,
    pub state: String,
}
#[cfg(test)]
impl std::fmt::Debug for AuthCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthCallback")
            .field("attempt", &self.attempt)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug)]
pub struct Receipt {
    pub checkin_id: String,
    pub account: Account,
    pub since: String,
    pub pickup: Option<Pickup>,
}
#[derive(Clone, Debug)]
pub struct Pickup {
    pub id: String,
    pub bytes: u64,
    pub expires: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SyncItem {
    /// Opaquer, vom Sync-Adapter vergebener Schlüssel, kein frei eingegebener Pfad.
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub selected: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncPlan {
    pub items: Vec<SyncItem>,
    pub excluded_bytes: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnFile {
    pub path: String,
    pub bytes: u64,
    pub status: String,
    pub large: bool,
    pub selected: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CheckoutPreview {
    pub id: String,
    pub personal: Vec<ReturnFile>,
    pub volatile_bytes: u64,
    pub volatile_files: usize,
    pub examples: Vec<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnSummary {
    pub files: usize,
    pub bytes: u64,
    pub conflicts: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub enum WaitTarget {
    Browser { attempt: u64 },
    Queue { attempt: u64, id: String },
    Transfer { checkin_id: String, id: String },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferKind {
    Upload,
    Checkout,
    Pickup,
}
#[derive(Clone, Debug)]
pub struct Progress {
    pub id: String,
    pub kind: TransferKind,
    pub bytes: u64,
    pub total: u64,
}
#[derive(Clone, Debug)]
pub enum FlowReply {
    Browser {
        attempt: u64,
        url: String,
    },
    Queue {
        attempt: u64,
        id: String,
        position: usize,
    },
    CheckedIn(Receipt),
    Plan(SyncPlan),
    Progress(Progress),
    Uploaded,
    Preview(CheckoutPreview),
    CheckedOut(ReturnSummary),
    Collected(ReturnSummary),
    Cancelled,
    Waiting,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    Start,
    Starting,
    Browser,
    Queue { id: String, position: usize },
    Pickup,
    Planning,
    Selection,
    Transfer(TransferKind),
    Working,
    PreviewLoading,
    Preview,
    Cancelling,
    Done,
}
#[derive(Clone, Debug)]
pub struct CancelTarget {
    pub attempt: u64,
    pub checkin_id: Option<String>,
    pub transfer_id: Option<String>,
    /// Empfang muss auch einen noch laufenden Check-in dieses Versuchs aufheben.
    pub abandon_checkin: bool,
}
#[derive(Clone, Debug)]
pub struct Flow {
    pub stage: Stage,
    pub attempt: u64,
    pub receipt: Option<Receipt>,
    pub plan: SyncPlan,
    pub preview: Option<CheckoutPreview>,
    pub progress: Option<Progress>,
    pub summary: Option<ReturnSummary>,
    pub notice: Option<String>,
    pub browser_url: Option<String>,
    resume_after_cancel: Stage,
}
impl Default for Flow {
    fn default() -> Self {
        Self {
            stage: Stage::Start,
            attempt: 0,
            receipt: None,
            plan: SyncPlan::default(),
            preview: None,
            progress: None,
            summary: None,
            notice: None,
            browser_url: None,
            resume_after_cancel: Stage::Start,
        }
    }
}
impl Flow {
    pub fn begin(&mut self, method: CheckinMethod) -> Option<Request> {
        if !matches!(self.stage, Stage::Start | Stage::Done) {
            return None;
        }
        let attempt = self.attempt + 1;
        *self = Self {
            attempt,
            stage: Stage::Starting,
            ..Self::default()
        };
        Some(Request::Checkin { attempt, method })
    }
    fn id(&self) -> Option<String> {
        self.receipt.as_ref().map(|r| r.checkin_id.clone())
    }
    pub fn plan(&mut self) -> Option<Request> {
        let id = self.id()?;
        self.stage = Stage::Planning;
        Some(Request::SyncPlan(id))
    }
    pub fn upload(&mut self) -> Option<Request> {
        if self.stage != Stage::Selection {
            return None;
        }
        let id = self.id()?;
        let selection = self
            .plan
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect::<Vec<_>>();
        if selection.is_empty() {
            return None;
        }
        self.stage = Stage::Transfer(TransferKind::Upload);
        self.progress = None;
        Some(Request::Upload {
            checkin_id: id,
            selection,
        })
    }
    pub fn without_files(&mut self) {
        if self.stage == Stage::Selection {
            self.stage = Stage::Working;
        }
    }
    pub fn preview(&mut self) -> Option<Request> {
        if !matches!(
            self.stage,
            Stage::Working | Stage::Selection | Stage::Planning | Stage::Pickup
        ) {
            return None;
        }
        let id = self.id()?;
        self.preview = None;
        self.stage = Stage::PreviewLoading;
        Some(Request::CheckoutPreview(id))
    }
    pub fn checkout(&mut self) -> Option<Request> {
        if self.stage != Stage::Preview {
            return None;
        }
        let checkin_id = self.id()?;
        let preview = self.preview.as_ref()?;
        let preview_id = preview.id.clone();
        if preview.personal.iter().any(|f| f.large && !f.selected) {
            return None;
        }
        let large_files = preview
            .personal
            .iter()
            .filter(|f| f.large && f.selected)
            .map(|f| f.path.clone())
            .collect();
        self.stage = Stage::Transfer(TransferKind::Checkout);
        self.progress = None;
        Some(Request::Checkout {
            checkin_id,
            preview_id,
            large_files,
        })
    }
    pub fn collect(&mut self) -> Option<Request> {
        if self.stage != Stage::Pickup {
            return None;
        }
        let receipt = self.receipt.as_ref()?;
        let pickup = receipt.pickup.as_ref()?;
        let request = Request::Collect {
            checkin_id: receipt.checkin_id.clone(),
            pickup_id: pickup.id.clone(),
        };
        self.stage = Stage::Transfer(TransferKind::Pickup);
        self.progress = None;
        Some(request)
    }
    pub fn poll(&self) -> Option<Request> {
        let target = match &self.stage {
            Stage::Browser => WaitTarget::Browser {
                attempt: self.attempt,
            },
            Stage::Queue { id, .. } => WaitTarget::Queue {
                attempt: self.attempt,
                id: id.clone(),
            },
            Stage::Transfer(_) => WaitTarget::Transfer {
                checkin_id: self.id()?,
                id: self.progress.as_ref()?.id.clone(),
            },
            _ => return None,
        };
        Some(Request::Wait(target))
    }
    pub fn cancel(&mut self) -> Option<Request> {
        if matches!(
            self.stage,
            Stage::Selection | Stage::Planning | Stage::Pickup
        ) {
            // Ein zugewiesener Platz wird niemals still freigegeben/gelöscht.
            return self.preview();
        }
        if self.stage == Stage::Preview || self.stage == Stage::PreviewLoading {
            self.stage = Stage::Working;
            self.preview = None;
            return None;
        }
        if !matches!(
            self.stage,
            Stage::Starting | Stage::Browser | Stage::Queue { .. } | Stage::Transfer(_)
        ) {
            return None;
        }
        let abandon_checkin = self.receipt.is_none();
        self.resume_after_cancel = match self.stage {
            Stage::Transfer(TransferKind::Upload) => Stage::Selection,
            Stage::Transfer(TransferKind::Pickup) => Stage::Pickup,
            Stage::Transfer(TransferKind::Checkout) => Stage::Working,
            _ => Stage::Start,
        };
        let target = CancelTarget {
            attempt: self.attempt,
            checkin_id: self.id(),
            transfer_id: self.progress.as_ref().map(|p| p.id.clone()),
            abandon_checkin,
        };
        self.stage = Stage::Cancelling;
        Some(Request::CancelFlow(target))
    }
    /// Nur zum aktuellen Zustand passende Bestätigungen ändern den Ablauf.
    pub fn apply(&mut self, reply: FlowReply) -> Option<Request> {
        match reply {
            FlowReply::Browser { attempt, url }
                if self.stage == Stage::Starting && attempt == self.attempt =>
            {
                self.stage = Stage::Browser;
                self.browser_url = Some(url);
            }
            FlowReply::Queue {
                attempt,
                id,
                position,
            } if attempt == self.attempt
                && matches!(
                    self.stage,
                    Stage::Starting | Stage::Browser | Stage::Queue { .. }
                ) =>
            {
                self.stage = Stage::Queue { id, position };
            }
            FlowReply::CheckedIn(receipt)
                if matches!(
                    self.stage,
                    Stage::Starting | Stage::Browser | Stage::Queue { .. }
                ) =>
            {
                self.receipt = Some(receipt);
                self.browser_url = None;
                if self.receipt.as_ref().is_some_and(|r| r.pickup.is_some()) {
                    self.stage = Stage::Pickup;
                } else {
                    return self.plan();
                }
            }
            FlowReply::Plan(plan) if self.stage == Stage::Planning => {
                self.plan = plan;
                self.stage = Stage::Selection;
            }
            FlowReply::Progress(progress) if self.stage == Stage::Transfer(progress.kind) => {
                self.progress = Some(progress);
            }
            FlowReply::Uploaded if self.stage == Stage::Transfer(TransferKind::Upload) => {
                self.stage = Stage::Working;
                self.progress = None;
            }
            FlowReply::Preview(preview) if self.stage == Stage::PreviewLoading => {
                self.preview = Some(preview);
                self.stage = Stage::Preview;
            }
            // Ein Abbruch kann zu spät kommen. Nur ein bestätigter Abschluss darf
            // dann Erfolg zeigen, niemals ein optimistisches lokales Löschen.
            FlowReply::CheckedOut(summary)
                if self.stage == Stage::Transfer(TransferKind::Checkout)
                    || (self.stage == Stage::Cancelling
                        && self.resume_after_cancel == Stage::Working) =>
            {
                if let Some(receipt) = &self.receipt {
                    self.notice = Some(format!(
                        "Ausgecheckt. {} Dateien in ~/cocopi · {} Konflikte erhalten. {} ist leer.",
                        summary.files, summary.conflicts, receipt.account.slot
                    ));
                }
                self.summary = Some(summary);
                self.stage = Stage::Done;
                self.receipt = None;
                self.progress = None;
            }
            FlowReply::Collected(summary)
                if self.stage == Stage::Transfer(TransferKind::Pickup) =>
            {
                self.notice = Some(format!(
                    "Dateien vom letzten Mal abgeholt · {} Dateien · {} Konflikte erhalten.",
                    summary.files, summary.conflicts
                ));
                if let Some(r) = &mut self.receipt {
                    r.pickup = None;
                }
                return self.plan();
            }
            FlowReply::Cancelled if self.stage == Stage::Cancelling => {
                self.stage = self.resume_after_cancel.clone();
                self.progress = None;
                self.notice = Some(if self.stage == Stage::Start { "Check-in abgebrochen." } else { "Abgebrochen. Der Arbeitsplatz bleibt belegt; deine Dateien bleiben auf R2, bis du auscheckst." }.into());
                if self.stage == Stage::Start {
                    self.receipt = None;
                    self.browser_url = None;
                }
            }
            _ => {}
        }
        None
    }
}

/// Ausschließlich explizit für Demo/Tests. Keine echte Anmeldung, keine I/O.
#[derive(Default)]
pub struct MemoryApi {
    state: Mutex<MemoryState>,
}
#[derive(Default)]
struct MemoryState {
    attempt: Option<u64>,
    queued: usize,
    authenticated: bool,
    occupied: bool,
    transfer: Option<Progress>,
    previewed: bool,
    selection: Vec<String>,
}
impl MemoryApi {
    #[cfg(test)]
    pub fn queued(position: usize) -> Self {
        Self {
            state: Mutex::new(MemoryState {
                queued: position,
                ..Default::default()
            }),
        }
    }
    pub fn receipt() -> Receipt {
        Receipt {
            checkin_id: "demo-checkin".into(),
            account: Account {
                person: "Mira".into(),
                slot: "account03".into(),
                device_name: "Miras MacBook".into(),
                device_id: "demo-desktop".into(),
            },
            since: "14:02".into(),
            pickup: None,
        }
    }
    pub fn sync_plan() -> SyncPlan {
        SyncPlan {
            items: vec![
                SyncItem {
                    id: "projekt-handout".into(),
                    name: "Projekte / Seminar-Handout · zuletzt benutzt".into(),
                    bytes: 4_200_000,
                    selected: true,
                },
                SyncItem {
                    id: "projekt-bilder".into(),
                    name: "Projekte / Bildreihe".into(),
                    bytes: 18_000_000,
                    selected: false,
                },
                SyncItem {
                    id: "skills".into(),
                    name: "Skills".into(),
                    bytes: 120_000,
                    selected: false,
                },
                SyncItem {
                    id: "verlaeufe".into(),
                    name: "Verläufe".into(),
                    bytes: 820_000,
                    selected: false,
                },
                SyncItem {
                    id: "ergebnisse".into(),
                    name: "Ergebnisse".into(),
                    bytes: 2_300_000,
                    selected: false,
                },
            ],
            excluded_bytes: 326_000_000,
        }
    }
    pub fn checkout_preview() -> CheckoutPreview {
        CheckoutPreview {
            id: "demo-preview".into(),
            personal: vec![
                ReturnFile {
                    path: "Ergebnisse/Seminar-Handout.pdf".into(),
                    bytes: 2_100_000,
                    status: "neu".into(),
                    large: false,
                    selected: true,
                },
                ReturnFile {
                    path: "Projekte/Seminar-Handout/gliederung.md".into(),
                    bytes: 12_000,
                    status: "geändert".into(),
                    large: false,
                    selected: true,
                },
                ReturnFile {
                    path: "Verläufe/Seminar-Handout.jsonl".into(),
                    bytes: 88_000,
                    status: "neu".into(),
                    large: false,
                    selected: true,
                },
            ],
            volatile_bytes: 326_000_000,
            volatile_files: 1240,
            examples: vec!["target/".into(), "node_modules/".into(), ".cache/".into()],
        }
    }
    pub fn summary() -> ReturnSummary {
        ReturnSummary {
            files: 3,
            bytes: 2_200_000,
            conflicts: 2,
        }
    }
}
impl DesktopApi for MemoryApi {
    fn call(&self, request: Request) -> Reply {
        let mut s = self.state.lock().unwrap();
        let fail = || {
            Reply::Error(InviteError::Other("Vorschau: Dieser Schritt passt nicht zum aktuellen Ablauf. Es wurden keine Daten verändert.".into()))
        };
        let reply = match request {
            Request::Checkin { attempt, method } => {
                s.attempt = Some(attempt);
                s.authenticated = false;
                match method {
                    CheckinMethod::Udk => FlowReply::Browser {
                        attempt,
                        url: "https://auth.example.invalid/cocopi-demo".into(),
                    },
                    CheckinMethod::Guest(code) if !code.trim().is_empty() => {
                        s.authenticated = true;
                        if s.queued > 0 {
                            FlowReply::Queue {
                                attempt,
                                id: "demo-queue".into(),
                                position: s.queued,
                            }
                        } else {
                            s.occupied = true;
                            FlowReply::CheckedIn(Self::receipt())
                        }
                    }
                    _ => return fail(),
                }
            }
            #[cfg(test)]
            Request::AuthCallback(callback)
                if s.attempt == Some(callback.attempt)
                    && callback.code == "mock-code"
                    && callback.state == "mock-state" =>
            {
                s.authenticated = true;
                FlowReply::Waiting
            }
            Request::Wait(WaitTarget::Browser { attempt })
            | Request::Wait(WaitTarget::Queue { attempt, .. })
                if s.attempt == Some(attempt) =>
            {
                if !s.authenticated {
                    FlowReply::Waiting
                } else if s.queued > 0 {
                    let position = s.queued;
                    s.queued -= 1;
                    FlowReply::Queue {
                        attempt,
                        id: "demo-queue".into(),
                        position,
                    }
                } else {
                    s.occupied = true;
                    FlowReply::CheckedIn(Self::receipt())
                }
            }
            Request::SyncPlan(id) if s.occupied && id == "demo-checkin" => {
                FlowReply::Plan(Self::sync_plan())
            }
            Request::Upload {
                checkin_id,
                selection,
            } if s.occupied && checkin_id == "demo-checkin" && !selection.is_empty() => {
                let plan = Self::sync_plan();
                if selection
                    .iter()
                    .any(|id| !plan.items.iter().any(|i| &i.id == id))
                {
                    return fail();
                }
                let total = plan
                    .items
                    .iter()
                    .filter(|i| selection.contains(&i.id))
                    .map(|i| i.bytes)
                    .sum();
                s.selection = selection;
                let p = Progress {
                    id: "demo-upload".into(),
                    kind: TransferKind::Upload,
                    bytes: 0,
                    total,
                };
                s.transfer = Some(p.clone());
                FlowReply::Progress(p)
            }
            Request::CheckoutPreview(id) if s.occupied && id == "demo-checkin" => {
                s.previewed = true;
                FlowReply::Preview(Self::checkout_preview())
            }
            Request::Checkout {
                checkin_id,
                preview_id,
                ..
            } if s.occupied
                && s.previewed
                && checkin_id == "demo-checkin"
                && preview_id == "demo-preview" =>
            {
                s.previewed = false;
                let p = Progress {
                    id: "demo-checkout".into(),
                    kind: TransferKind::Checkout,
                    bytes: 0,
                    total: Self::summary().bytes,
                };
                s.transfer = Some(p.clone());
                FlowReply::Progress(p)
            }
            Request::Collect { checkin_id, .. } if s.occupied && checkin_id == "demo-checkin" => {
                let p = Progress {
                    id: "demo-pickup".into(),
                    kind: TransferKind::Pickup,
                    bytes: 0,
                    total: 2_200_000,
                };
                s.transfer = Some(p.clone());
                FlowReply::Progress(p)
            }
            Request::Wait(WaitTarget::Transfer { checkin_id, id })
                if checkin_id == "demo-checkin" && s.occupied =>
            {
                let Some(p) = &mut s.transfer else {
                    return fail();
                };
                if p.id != id {
                    return fail();
                }
                if p.bytes < p.total {
                    p.bytes = (p.bytes + p.total.div_ceil(4)).min(p.total);
                    FlowReply::Progress(p.clone())
                } else {
                    let kind = p.kind;
                    s.transfer = None;
                    match kind {
                        TransferKind::Upload => FlowReply::Uploaded,
                        TransferKind::Pickup => FlowReply::Collected(Self::summary()),
                        TransferKind::Checkout => {
                            s.occupied = false;
                            s.attempt = None;
                            FlowReply::CheckedOut(Self::summary())
                        }
                    }
                }
            }
            Request::CancelFlow(target) if s.attempt == Some(target.attempt) => {
                s.transfer = None;
                if target.abandon_checkin {
                    s.attempt = None;
                    s.occupied = false;
                    s.authenticated = false;
                }
                FlowReply::Cancelled
            }
            _ => return fail(),
        };
        Reply::Flow(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(api: &MemoryApi, flow: &mut Flow, request: Request) {
        let Reply::Flow(reply) = api.call(request) else {
            panic!("Fixture-Anfrage abgewiesen")
        };
        if let Some(next) = flow.apply(reply) {
            apply(api, flow, next);
        }
    }
    fn entered() -> (MemoryApi, Flow) {
        let api = MemoryApi::default();
        let mut flow = Flow::default();
        let request = flow.begin(CheckinMethod::Guest("mock".into())).unwrap();
        apply(&api, &mut flow, request);
        (api, flow)
    }
    #[test]
    fn no_upload_without_confirmation_and_only_last_project_selected() {
        let (api, flow) = entered();
        assert_eq!(flow.stage, Stage::Selection);
        assert_eq!(flow.plan.items.iter().filter(|i| i.selected).count(), 1);
        assert!(api.state.lock().unwrap().selection.is_empty());
        assert!(api.state.lock().unwrap().transfer.is_none());
        let mut flow = flow;
        flow.without_files();
        assert_eq!(flow.stage, Stage::Working);
        assert!(api.state.lock().unwrap().selection.is_empty());
    }
    #[test]
    fn complete_upload_preview_checkout_and_conflicts() {
        let (api, mut flow) = entered();
        let request = flow.upload().unwrap();
        apply(&api, &mut flow, request);
        while let Some(request) = flow.poll() {
            apply(&api, &mut flow, request);
        }
        assert_eq!(flow.stage, Stage::Working);
        assert_eq!(api.state.lock().unwrap().selection, ["projekt-handout"]);
        let request = flow.preview().unwrap();
        apply(&api, &mut flow, request);
        let request = flow.checkout().unwrap();
        apply(&api, &mut flow, request);
        while let Some(request) = flow.poll() {
            apply(&api, &mut flow, request);
        }
        assert_eq!(flow.stage, Stage::Done);
        assert_eq!(flow.summary.unwrap().conflicts, 2);
        assert!(!api.state.lock().unwrap().occupied);
    }
    #[test]
    fn checkout_requires_current_preview_in_ui_and_adapter() {
        let (api, mut flow) = entered();
        assert!(flow.checkout().is_none());
        flow.without_files();
        assert!(flow.checkout().is_none());
        assert!(matches!(
            api.call(Request::Checkout {
                checkin_id: "demo-checkin".into(),
                preview_id: "demo-preview".into(),
                large_files: vec![],
            }),
            Reply::Error(_)
        ));
        flow.apply(FlowReply::CheckedOut(MemoryApi::summary()));
        assert_eq!(flow.stage, Stage::Working);
    }
    #[test]
    fn browser_callback_is_not_an_identity_and_queue_advances_without_click() {
        let api = MemoryApi::queued(2);
        let mut flow = Flow::default();
        let request = flow.begin(CheckinMethod::Udk).unwrap();
        apply(&api, &mut flow, request);
        assert_eq!(flow.stage, Stage::Browser);
        assert!(flow.receipt.is_none());
        let bad = AuthCallback {
            attempt: flow.attempt,
            code: "fake".into(),
            state: "mock-state".into(),
        };
        assert!(matches!(
            api.call(Request::AuthCallback(bad)),
            Reply::Error(_)
        ));
        let request = flow.poll().unwrap();
        apply(&api, &mut flow, request);
        assert_eq!(flow.stage, Stage::Browser);
        let callback = AuthCallback {
            attempt: flow.attempt,
            code: "mock-code".into(),
            state: "mock-state".into(),
        };
        assert!(matches!(
            api.call(Request::AuthCallback(callback)),
            Reply::Flow(FlowReply::Waiting)
        ));
        for position in [2, 1] {
            let request = flow.poll().unwrap();
            apply(&api, &mut flow, request);
            assert!(matches!(flow.stage, Stage::Queue { position: p, .. } if p == position));
        }
        let request = flow.poll().unwrap();
        apply(&api, &mut flow, request);
        assert_eq!(flow.stage, Stage::Selection);
    }
    #[test]
    fn cancel_each_step_preserves_remote_data_until_confirmed_checkout() {
        for stage in [
            Stage::Starting,
            Stage::Browser,
            Stage::Queue {
                id: "demo-queue".into(),
                position: 2,
            },
            Stage::Transfer(TransferKind::Upload),
            Stage::Transfer(TransferKind::Checkout),
            Stage::Transfer(TransferKind::Pickup),
        ] {
            let (api, mut flow) = entered();
            let before_checkin = matches!(
                stage,
                Stage::Starting | Stage::Browser | Stage::Queue { .. }
            );
            if before_checkin {
                flow.receipt = None;
            }
            flow.stage = stage;
            let request = flow.cancel().unwrap();
            assert_eq!(flow.stage, Stage::Cancelling);
            apply(&api, &mut flow, request);
            assert_ne!(flow.stage, Stage::Done);
            assert_eq!(api.state.lock().unwrap().occupied, !before_checkin);
        }
        for stage in [Stage::Selection, Stage::Planning, Stage::Pickup] {
            let (api, mut flow) = entered();
            flow.stage = stage;
            let request = flow.cancel().unwrap();
            apply(&api, &mut flow, request);
            assert_eq!(flow.stage, Stage::Preview);
            assert!(api.state.lock().unwrap().occupied);
            assert!(flow.cancel().is_none());
            assert_eq!(flow.stage, Stage::Working);
        }
        let (_, mut flow) = entered();
        flow.stage = Stage::PreviewLoading;
        flow.cancel();
        assert_eq!(flow.stage, Stage::Working);
        flow.apply(FlowReply::Preview(MemoryApi::checkout_preview()));
        assert!(flow.preview.is_none());
    }
    #[test]
    fn cancelled_checkin_cannot_revive_from_late_reply() {
        let (api, mut flow) = entered();
        flow.receipt = None;
        flow.stage = Stage::Browser;
        let request = flow.cancel().unwrap();
        apply(&api, &mut flow, request);
        flow.apply(FlowReply::CheckedIn(MemoryApi::receipt()));
        assert_eq!(flow.stage, Stage::Start);
        assert!(flow.receipt.is_none());
    }
    #[test]
    fn pickup_is_offered_before_selection_and_can_be_collected() {
        let (api, mut flow) = entered();
        flow.stage = Stage::Browser;
        let mut receipt = MemoryApi::receipt();
        receipt.pickup = Some(Pickup {
            id: "pickup".into(),
            bytes: 2_200_000,
            expires: "morgen".into(),
        });
        flow.apply(FlowReply::CheckedIn(receipt));
        assert_eq!(flow.stage, Stage::Pickup);
        let request = flow.collect().unwrap();
        apply(&api, &mut flow, request);
        while let Some(request) = flow.poll() {
            apply(&api, &mut flow, request);
        }
        assert_eq!(flow.stage, Stage::Selection);
        assert!(flow.notice.unwrap().contains("abgeholt"));
    }
}
