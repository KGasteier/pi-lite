# Mitgelieferte Bibliotheken (`ui/vendor/`)

Die Dateien sind unverändert aus den jeweiligen Release-Paketen kopiert.

| Datei | Projekt | Version | Lizenz |
|---|---|---|---|
| `marked.js` | [marked](https://github.com/markedjs/marked) | 18.1.0 | MIT |
| `purify.js` | [DOMPurify](https://github.com/cure53/DOMPurify) | 3.4.16 | Apache-2.0 oder MPL-2.0 |
| `hljs.js`, `hljs.css` | [highlight.js](https://github.com/highlightjs/highlight.js) | 11.12.0 | BSD-3-Clause |

Rust-Abhängigkeiten (Tauri, serde) stehen in `src-tauri/Cargo.lock`; Tauri ist MIT/Apache-2.0.

Vor einer Veröffentlichung: Lizenztexte der Vendor-Dateien als Dateien beilegen (siehe `docs/PUBLISHING.md`).
