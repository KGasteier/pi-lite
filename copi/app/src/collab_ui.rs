use crate::app::App;
use pi_net::{Card,Kind,Node,Event,Space,SpaceKind};
#[derive(Default)]
pub struct Collab {
    node:Option<Node>, address:String, contact:String, status:String,
    draft:String, kind:usize, confirm:bool, cards:Vec<Card>, name:String,
    spaces:Vec<Space>, active:usize, received:Option<String>, adopt:bool,
}
impl App {
    pub(crate) fn collaboration_panel(&mut self,ui:&mut egui::Ui){
        let c=&mut self.collab;
        if let Some(n)=&c.node {while let Ok(e)=n.events.try_recv(){ match e {
            Event::Address(a)=>{c.address=a;c.status="Direkttransport gestartet · flüchtiger Geräteschlüssel".into();}
            Event::Connected(id)=>c.status=format!("Kontakt gewählt: {} · Verbindung erst beim Senden",id),
            Event::Received(card)=>{c.status="Karte empfangen · keine KI-Übertragung".into();c.cards.push(card);}
            Event::Sent=>c.status="Empfang auf Gegenseite bestätigt".into(),
            Event::Error(e)=>c.status=e, Event::Stopped=>c.status="Transport beendet".into(),
        }}}
        ui.horizontal(|ui|{ui.colored_label(crate::theme::OK,"cocopi · Netz");if ui.small_button("×").clicked(){self.store.net_visible=false;}});
        ui.label("P2P-Prototyp · kein MLS / kein UdK-Ausweis");
        egui::ScrollArea::vertical().id_salt("cocopi-network").show(ui,|ui|{
            ui.style_mut().visuals.selection.bg_fill = crate::theme::OK;
            ui.label("Noch nicht für vertrauliche Seminararbeit freigegeben. Inhalte nur im RAM; keine Dateien, kein Offline-Postfach.");
            if c.node.is_none(){
                if ui.add_enabled(!self.demo,egui::Button::new("Direkttransport ausdrücklich starten")).clicked(){c.node=Some(Node::start());c.status="Startet …".into();}
            }else if ui.button("Transport stoppen").clicked(){c.node=None;c.address.clear();c.status="Gestoppt".into();}
            ui.label(&c.status);
            if !c.address.is_empty(){
                ui.label("Eigene Kontaktadresse (enthält IP-Adressen):");
                if ui.button("Adresse kopieren").clicked(){ui.ctx().copy_text(c.address.clone());}
                ui.label("Beide Personen tauschen Adressen über einen vertrauenswürdigen Kanal aus und bestätigen den vollständigen Geräteschlüssel. Keine Einmal-Einladung!");
                ui.add(egui::TextEdit::multiline(&mut c.contact).hint_text("Kontaktadresse als JSON").desired_rows(2));
                if ui.button("Diesen Kontakt ausdrücklich bestätigen").clicked(){
                    if let Some(n)=&c.node{if let Err(e)=n.pair(c.contact.clone()){c.status=e;}}
                }
            }
            ui.separator();ui.label("Projekte / Seminare · lokale Planungsräume");
            ui.text_edit_singleline(&mut c.name);
            ui.horizontal(|ui|{
                if ui.button("Projekt anlegen").clicked(){match Space::create(&c.name,SpaceKind::Project,false){Ok(s)=>{c.spaces.push(s);c.name.clear();},Err(e)=>c.status=e}}
                ui.add_enabled(false,egui::Button::new("Seminar")).on_hover_text("Benötigt künftig signierten authentik-Lehrendenausweis. Keine lokale Rollen-Selbstvergabe.");
            });
            for (i,s) in c.spaces.iter().enumerate(){ui.selectable_value(&mut c.active,i,&s.name);}
            if let Some(s)=c.spaces.get_mut(c.active){
                ui.label("Nur lokaler Ablaufentwurf, noch keine gemeinsame Gruppe.");
                ui.checkbox(&mut s.follow,"Tutorial folgen (lokal)");ui.checkbox(&mut s.hand,"Hand heben (lokal)");
                ui.horizontal(|ui|{ui.label(format!("Schritt {}",s.step));if ui.small_button("Weiter").clicked(){s.step+=1;}});
            }
            ui.separator();
            ui.horizontal(|ui|{ui.selectable_value(&mut c.kind,0,"Chat");ui.selectable_value(&mut c.kind,1,"Code");ui.selectable_value(&mut c.kind,2,"Prompt");});
            ui.add(egui::TextEdit::multiline(&mut c.draft).desired_rows(3).hint_text("Bewusst an bestätigten Direktkontakt teilen"));
            if ui.add_enabled(c.node.is_some(),egui::Button::new("Senden prüfen …")).clicked(){c.confirm=true;}
            if c.confirm{
                ui.label("Diesen Inhalt an den gewählten Kontakt senden? Die Gegenseite kann ihn kopieren. Kein Agent wird gestartet.");
                ui.horizontal(|ui|{
                    if ui.button("Jetzt übertragen").clicked(){
                        let kind=match c.kind{1=>Kind::Code,2=>Kind::Prompt,_=>Kind::Chat};
                        match Card::new(kind,&c.draft).and_then(|card|c.node.as_ref().ok_or_else(||"Transport aus".to_string())?.send(card)){
                            Ok(())=>{c.draft.clear();c.confirm=false;},Err(e)=>c.status=e,
                        }
                    }
                    if ui.button("Abbrechen").clicked(){c.confirm=false;}
                });
            }
            for (i,card) in c.cards.iter().enumerate(){
                ui.push_id(i,|ui|{egui::Frame::new().stroke(egui::Stroke::new(1.0,crate::theme::OK)).inner_margin(8.0).show(ui,|ui|{
                    ui.colored_label(crate::theme::OK,format!("Empfangen · {:?}",card.kind));ui.label(&card.text);
                    if ui.small_button("Kopieren").clicked(){ui.ctx().copy_text(card.text.clone());}
                    if card.kind==Kind::Prompt && ui.button("In Eingabe übernehmen …").clicked(){c.received=Some(card.text.clone());c.adopt=true;}
                });});
            }
            if c.adopt{
                ui.label("Ersetzt die aktuelle Eingabe. Erst dein späteres Senden überträgt den fremden Inhalt an das ausgewählte KI-Modell. Vorher prüfen!");
                ui.horizontal(|ui|{
                    if ui.button("Nur in editierbare Eingabe übernehmen").clicked(){if let Some(p)=c.received.take(){self.prompt=p;self.focus_prompt=true;}c.adopt=false;}
                    if ui.button("Verwerfen").clicked(){c.received=None;c.adopt=false;}
                });
            }
        });
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
}
