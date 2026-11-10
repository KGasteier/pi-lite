// Kontextmanager (pi-lite): Suche/Ordnung als reine Funktionen und Struktur der Seitenleiste.
// Automatisiert mit Node, kein Fenster, kein Live-Test.
const fs = require("node:fs"), assert = require("node:assert/strict");
const CM = require("../ui/context-manager.js");

assert.deepEqual(CM.searchWords("  Foo BAR  baz "), ["foo", "bar", "baz"]);
assert.equal(CM.searchWords("a b c d e f g h i j").length, 8);

const local = { path: "/s/a.jsonl", name: "Gliederung", title: "", cwd: "/home/kg/Work/pi-lite", modified: 10 };
const other = { path: "/s/b.jsonl", name: "", title: "Tutorial", cwd: "/tmp", modified: 20 };
const remote = { path: "/r/c.jsonl", name: "", title: "Server", cwd: "/srv", modified: 30, backend: "r2", backendLabel: "R2" };
const hits = { "/s/b.jsonl": { counts: [2, 0], snippet: "… repair hier …" }, "/r/c.jsonl": { counts: [5, 5], snippet: "x" } };
// UND-Verknüpfung über Metadaten + Inhaltstreffer.
assert(CM.searchMatch(local, ["gliederung", "pi-lite"], {}));
assert(!CM.searchMatch(local, ["gliederung", "fehlt"], {}));
assert(CM.searchMatch(other, ["repair", "tutorial"], hits), "Wort 1 im Inhalt, Wort 2 im Titel");
assert(!CM.searchMatch(other, ["repair", "zweites"], hits));
// Remote: nur Metadaten, Inhaltstreffer werden ignoriert.
assert(CM.searchMatch(remote, ["r2"], hits));
assert(!CM.searchMatch(remote, ["repair"], hits));
assert(CM.searchMatch(remote, [], hits));

const ids = list => list.map(s => s.path);
assert.deepEqual(ids(CM.order([local, other, remote])), ["/r/c.jsonl", "/s/b.jsonl", "/s/a.jsonl"]);
assert.deepEqual(ids(CM.order([local, other, remote], { inverse: true })), ["/s/a.jsonl", "/s/b.jsonl", "/r/c.jsonl"]);
const rank = { "/s/a.jsonl": 0, "/s/b.jsonl": 1, "remote:r2:/r/c.jsonl": 1 };
assert.deepEqual(ids(CM.order([local, other, remote], { colorRank: s => rank[s.backend ? `remote:${s.backend}:${s.path}` : s.path] })), ["/s/a.jsonl", "/r/c.jsonl", "/s/b.jsonl"]);
assert.equal(CM.hitLine(hits["/s/b.jsonl"], ["repair", "x"]), "repair 2× · x 0× – … repair hier …");
assert.equal(CM.tokenShort(22000), "22k"); assert.equal(CM.tokenShort(22400), "22.4k"); assert.equal(CM.tokenShort(512), "512");
assert.deepEqual(Object.keys(CM.BOOKMARK_LABELS), ["off", "on", "colors"]);

// Struktur: Kopf „Kontextmanager“, Umschalter, Suche, vier Schalter, Fußzeile mit Import/Export.
const html = fs.readFileSync("ui/index.html", "utf8");
const side = html.slice(html.indexOf('<aside id="side">'), html.indexOf("</aside>"));
assert(side.includes(">Kontextmanager<") && side.includes('id="newsess"'));
const order = ["kmswitch", "kmsearch", "sidetools", "sessions", "outline", "sidefoot"].map(id => side.indexOf(`id="${id}"`));
assert(order.every(i => i > 0) && order.every((v, i) => !i || v > order[i - 1]), "Reihenfolge der Seitenleiste");
const tools = side.slice(side.indexOf('id="sidetools"'), side.indexOf('id="libcats"'));
assert.deepEqual([...tools.matchAll(/id="([^"]+)" class="chip"/g)].map(m => m[1]), ["outlinetoggle", "timeorder", "bookmarkfilter", "originfilter"]);
assert(side.includes('id="ioimport"') && side.includes('id="ioexport"') && side.includes('id="iorepair"'));
assert(html.includes('<div id="repair" hidden></div>') && html.includes('<script src="context-manager.js"></script>'));
assert(html.indexOf("context-manager.js") < html.indexOf('src="app.js"'));
const app = fs.readFileSync("ui/app.js", "utf8");
assert(app.includes('invoke("session_search"') && app.includes('"import_md"') && app.includes("createSessionCard(s, { outline: true })"));
const cm = fs.readFileSync("ui/context-manager.js", "utf8");
for (const c of ["repair_analyze", "repair_apply", "repair_undo", "repair_export"]) assert(cm.includes(`"${c}"`), c);
assert(cm.includes("GEPLANT, NICHT IMPLEMENTIERT"), "Remote-Repair klar als geplant markiert");
assert(!cm.includes("−-"), "kein „−-“ im Anwenden-Knopf");
const rs = fs.readFileSync("src-tauri/src/main.rs", "utf8");
for (const c of ["session_search", "repair_analyze", "repair_apply", "repair_undo", "repair_export", "import_md"]) assert(rs.includes(`context::${c}`), c);
console.log("Kontextmanager (pi-lite): Suche/Ordnung/Trefferzeile, Seitenleisten-Struktur, Repair- und Tauri-Befehle registriert; kein Live-Test.");
