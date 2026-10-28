# Mitgelieferte Bibliotheken (`ui/vendor/`)

Die Dateien sind unverändert aus den jeweiligen Release-Paketen kopiert.

| Datei | Projekt | Version | Lizenz |
|---|---|---|---|
| `marked.js` | [marked](https://github.com/markedjs/marked) | 18.1.0 | MIT |
| `purify.js` | [DOMPurify](https://github.com/cure53/DOMPurify) | 3.4.16 | Apache-2.0 oder MPL-2.0 |
| `hljs.js`, `hljs.css` | [highlight.js](https://github.com/highlightjs/highlight.js) | 11.12.0 | BSD-3-Clause |

Rust-Abhängigkeiten (Tauri, serde) stehen in `src-tauri/Cargo.lock`; Tauri ist MIT/Apache-2.0.

Vor einer Veröffentlichung: Lizenztexte der Vendor-Dateien als Dateien beilegen (siehe `docs/PUBLISHING.md`).

## cocopi v3 · Desktop

### Orca (MIT), Referenz `05aab3a185`

Quelle: <https://github.com/stablyai/orca>, Commit `05aab3a185`.
Die Desktop-Abläufe für Geräte-Pairing und die Zustandsbegriffe wurden unabhängig
in Rust/egui umgesetzt; Orca diente als konzeptionelle Referenz, kein TypeScript
wurde unverändert eingebettet. Vorsorglicher Copyright- und Lizenzhinweis:

```text
MIT License

Copyright (c) 2026 Lovecast Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### qrcodegen 1.8.0 (MIT)

Quelle: <https://www.nayuki.io/page/qr-code-generator-library>, Rust-Crate
`qrcodegen` 1.8.0. Verwendet für die echte QR-Matrix in der egui-Textur (keine
Kamera, kein externer QR-Dienst). Lizenztext aus `qrcodegen-1.8.0/src/lib.rs`:

```text
Copyright (c) Project Nayuki. (MIT License)

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:
- The above copyright notice and this permission notice shall be included in
  all copies or substantial portions of the Software.
- The Software is provided "as is", without warranty of any kind, express or
  implied, including but not limited to the warranties of merchantability,
  fitness for a particular purpose and noninfringement. In no event shall the
  authors or copyright holders be liable for any claim, damages or other
  liability, whether in an action of contract, tort or otherwise, arising from,
  out of or in connection with the Software or the use or other dealings in the
  Software.
```
