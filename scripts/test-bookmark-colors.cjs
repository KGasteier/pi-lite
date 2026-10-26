const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const path = require('node:path');

// ---------- ui/bookmark-palette.js ----------
const { BookmarkPalette, BOOKMARK_PALETTE_COLORS, normalizeBookmarkColor, bookmarkColorHex, bookmarkColorName } = require('../ui/bookmark-palette.js');

assert.equal(BOOKMARK_PALETTE_COLORS.length, 10, 'Palette muss genau zehn Farben haben');
assert.equal(new Set(BOOKMARK_PALETTE_COLORS.map(c => c.name)).size, 10, 'Farbnamen müssen eindeutig sein');
assert.equal(new Set(BOOKMARK_PALETTE_COLORS.map(c => c.hex)).size, 10, 'Hex-Codes müssen eindeutig sein');
for (const c of BOOKMARK_PALETTE_COLORS) assert.match(c.hex, /^#[0-9a-f]{6}$/i, `ungültiger Hex-Code: ${c.hex}`);
assert.throws(() => { BOOKMARK_PALETTE_COLORS.push({ name: 'x', hex: '#000000' }); }, 'Palette sollte eingefroren (Object.freeze) sein');
BOOKMARK_PALETTE_COLORS[0].hex = '#000000'; // kein Strict-Mode hier -> stiller No-Op statt Exception
assert.equal(BOOKMARK_PALETTE_COLORS[0].hex, '#6aa5e8', 'einzelne Einträge sollten eingefroren sein (Mutation ohne Wirkung)');

assert.equal(normalizeBookmarkColor(0), 0);
assert.equal(normalizeBookmarkColor(7), 7);
assert.equal(normalizeBookmarkColor(8), 8, 'angehängte Farbe 9 gültig');
assert.equal(normalizeBookmarkColor(9), 9, 'angehängte Farbe 10 gültig');
assert.equal(normalizeBookmarkColor(10), 0, 'außerhalb der Palette -> Vorgabe');
// Regression: Anhängen darf alte Indizes 0..7 nicht verschieben.
assert.deepEqual(BOOKMARK_PALETTE_COLORS.slice(0, 8).map(c => c.hex), ['#6aa5e8', '#5ec2a0', '#e0b25a', '#b08cf0', '#e68f6e', '#ef476f', '#18a999', '#e454c4']);
assert.equal(normalizeBookmarkColor(-1), 0);
assert.equal(normalizeBookmarkColor(1.5), 0, 'keine Fließkommazahl als Index');
assert.equal(normalizeBookmarkColor('3'), 0, 'kein String als Index');
assert.equal(normalizeBookmarkColor(undefined), 0);
assert.equal(normalizeBookmarkColor(NaN), 0);

assert.equal(bookmarkColorHex(0), BOOKMARK_PALETTE_COLORS[0].hex);
assert.equal(bookmarkColorHex(99), BOOKMARK_PALETTE_COLORS[0].hex, 'ungültiger Index fällt auf Vorgabefarbe zurück');
assert.equal(bookmarkColorName(2), BOOKMARK_PALETTE_COLORS[2].name);
assert.equal(bookmarkColorName(-5), BOOKMARK_PALETTE_COLORS[0].name);
assert.equal(BookmarkPalette.hex(1), bookmarkColorHex(1), 'Namespace-Objekt und freie Funktionen müssen übereinstimmen');

// Parität mit der Rust-Palette (copi/app/src/bookmark_palette.rs): identische Hex-Codes,
// Namen und Reihenfolge – ohne das Rust-Modul hier zu parsen, aber als harter String-Abgleich
// der Quelle, damit ein Abweichen zwischen beiden Implementierungen sofort auffällt.
const rustSrc = fs.readFileSync(path.join(__dirname, '../copi/app/src/bookmark_palette.rs'), 'utf8');
for (const c of BOOKMARK_PALETTE_COLORS) {
  assert(rustSrc.includes(`name: "${c.name}", hex: "${c.hex}"`), `Rust-Palette fehlt oder weicht ab für ${c.name}/${c.hex}`);
}

// ---------- ui/session-bookmarks.js (Farbpersistenz) ----------
const context = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../ui/session-bookmarks.js'), 'utf8'), context);
const data = new Map(), storage = { getItem: k => data.get(k) || null, setItem: (k, v) => data.set(k, v) };
let bm = context.createSessionBookmarks(storage);

// Farbe eines nicht markierten Pfads ist immer 0, Setzen hat keine Wirkung.
assert.equal(bm.colorOf('x'), 0);
assert.equal(bm.setColor('x', 3), false, 'setColor an nicht markiertem Pfad soll fehlschlagen');
assert.equal(bm.colorOf('x'), 0);

bm.toggle('x');
assert.equal(bm.colorOf('x'), 0, 'frisch markierter Pfad startet mit Vorgabefarbe');
assert.equal(bm.setColor('x', 4), true);
assert.equal(bm.colorOf('x'), 4);

// Persistenz über eine neue Instanz (wie ein Neustart der App).
let bm2 = context.createSessionBookmarks(storage);
assert.equal(bm2.colorOf('x'), 4, 'Farbe muss über einen Neustart hinweg erhalten bleiben');
assert.equal(bm2.has('x'), true);

// Ungültiger Index normalisiert auf 0 statt eine kaputte Farbe zu speichern.
assert.equal(bm2.setColor('x', 99), true);
assert.equal(bm2.colorOf('x'), 0, 'außerhalb der Palette -> Vorgabefarbe');

// Entfernen des Bookmarks räumt die Farbe auf (kein Geisterzustand).
bm2.setColor('x', 2);
assert.equal(bm2.toggle('x'), false, 'toggle entfernt das Bookmark');
assert.equal(bm2.colorOf('x'), 0);
let bm3 = context.createSessionBookmarks(storage);
assert.equal(bm3.has('x'), false);
assert.equal(bm3.colorOf('x'), 0, 'Farbe eines entfernten Bookmarks darf nach Neustart nicht wieder auftauchen');

// Mehrere Pfade unabhängig voneinander.
bm3.toggle('a'); bm3.toggle('b');
bm3.setColor('a', 1); bm3.setColor('b', 7);
assert.equal(bm3.colorOf('a'), 1);
assert.equal(bm3.colorOf('b'), 7);
bm3.toggle('a'); // nur a entfernen
assert.equal(bm3.colorOf('a'), 0);
assert.equal(bm3.colorOf('b'), 7, 'Entfernen eines Bookmarks darf andere Bookmark-Farben nicht beeinflussen');

// Beschädigter/fehlender Farbspeicher darf die Grundfunktion (Bookmarks selbst) nicht
// zerstören – wie beim bestehenden Test für den Hauptspeicher in test-session-bookmarks.cjs.
const brokenColorsData = new Map([['pi-lite.session-bookmarks.v1', '["y"]'], ['pi-lite.session-bookmark-colors.v1', '{not valid json']]);
const brokenColorsStorage = { getItem: k => brokenColorsData.get(k) || null, setItem: (k, v) => brokenColorsData.set(k, v) };
const bmBrokenColors = context.createSessionBookmarks(brokenColorsStorage);
// Ein kaputter Farbspeicher darf die bereits vorher geparsten Bookmarks selbst nicht
// zerstören (nur die Farbzuordnung fällt auf die Vorgabe zurück) – Bookmarks bleiben also
// funktionsfähig, nur ohne Farbe.
assert.equal(bmBrokenColors.has('y'), true, 'Bookmark selbst bleibt trotz kaputtem Farbspeicher erhalten');
assert.equal(bmBrokenColors.colorOf('y'), 0, 'Farbe fällt bei kaputtem Farbspeicher auf die Vorgabe zurück');

// Farbspeicher, der kein Objekt ist (z. B. ein Array oder eine Zahl), wird ignoriert statt
// einen Fehler zu werfen.
const arrayColorsData = new Map([['pi-lite.session-bookmarks.v1', '["z"]'], ['pi-lite.session-bookmark-colors.v1', '[1,2,3]']]);
const arrayColorsStorage = { getItem: k => arrayColorsData.get(k) || null, setItem: (k, v) => arrayColorsData.set(k, v) };
const bmArrayColors = context.createSessionBookmarks(arrayColorsStorage);
assert.equal(bmArrayColors.has('z'), true, 'Bookmarks bleiben auch bei falsch typisiertem Farbspeicher erhalten');
assert.equal(bmArrayColors.colorOf('z'), 0);

// Farbeinträge für inzwischen nicht mehr markierte Pfade (z. B. durch manuell editierten
// localStorage) werden beim Laden verworfen statt verwaist zu bleiben.
const orphanData = new Map([['pi-lite.session-bookmarks.v1', '["kept"]'], ['pi-lite.session-bookmark-colors.v1', '{"kept":3,"removed-long-ago":5}']]);
const orphanStorage = { getItem: k => orphanData.get(k) || null, setItem: (k, v) => orphanData.set(k, v) };
const bmOrphan = context.createSessionBookmarks(orphanStorage);
assert.equal(bmOrphan.colorOf('kept'), 3);
assert.equal(bmOrphan.colorOf('removed-long-ago'), 0);

console.log('Bookmark-Farben: Palette (10 feste Farben, Index-Normalisierung, Rust-Parität) sowie Farbpersistenz/-aufräumen in session-bookmarks.js passed.');
