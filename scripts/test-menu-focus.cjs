const assert=require('node:assert/strict'),fs=require('node:fs');
const js=fs.readFileSync('ui/app.js','utf8'),html=fs.readFileSync('ui/index.html','utf8'),rust=fs.readFileSync('copi/app/src/view.rs','utf8');
assert(js.includes('input.onkeydown = e =>'));assert(js.includes('m.querySelectorAll("input").forEach'));assert(js.includes('command.skillGroup'));
assert(html.includes('>/ Befehle</button>'));assert(html.includes('font-style:italic'));assert(html.includes('right:56px; bottom:5px'));
assert(rust.includes('PopupCloseBehavior::CloseOnClickOutside'));assert(rust.includes('!egui::Popup::is_any_open(ctx)'));assert(rust.includes('!ctx.memory(|m| m.focused().is_some())'));assert(rust.includes('"card-delete"'));assert(rust.includes('RichText::new("/skill:").italics()'));
console.log('Menü-Fokus, Abschluss, Aktionsreihenfolge und Befehlskennzeichnung: Struktur geprüft. Kein Live-Test.');
