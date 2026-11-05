// Zehn feste Bookmark-Standardfarben für Session-Lesezeichen (pi-lite).
//
// Getrennt von session-bookmarks.js, weil diese Palette ausschließlich Hex-Codes/Namen
// definiert und von mehreren Stellen gebraucht wird (Sidebar-Karte, Kontextmenü,
// Farbauswahl-Button). Die Werte sind bewusst 1:1 identisch zu
// copi/app/src/bookmark_palette.rs (gleiche Hex-Codes, gleiche Reihenfolge, gleiche
// deutsche Namen). index.html lädt die Palette vor session-bookmarks.js; app.js nutzt
// sie für die Session-Karten und den Farbwähler.
//
// Index 0 ist die Standardfarbe neuer Bookmarks (entspricht der bisherigen, einfarbigen
// Kategoriefarbe --cat), Indizes 1..=9 sind (8 und 9 nachträglich angehängt – nie umsortieren) frei wählbare Zusatzfarben. Ein ungültiger
// gespeicherter Index (z. B. durch manuell editierten localStorage-Inhalt oder eine
// künftig verkleinerte Palette) fällt sicher auf Index 0 zurück statt eine ungültige
// Farbe zu rendern.
const BOOKMARK_PALETTE_COLORS = Object.freeze([
  Object.freeze({ name: "Blau", hex: "#6aa5e8" }),
  Object.freeze({ name: "Grün", hex: "#5ec2a0" }),
  Object.freeze({ name: "Gelb", hex: "#e0b25a" }),
  Object.freeze({ name: "Violett", hex: "#b08cf0" }),
  Object.freeze({ name: "Orange", hex: "#e68f6e" }),
  Object.freeze({ name: "Rot", hex: "#ef476f" }),
  Object.freeze({ name: "Türkis", hex: "#18a999" }),
  Object.freeze({ name: "Pink", hex: "#e454c4" }),
  Object.freeze({ name: "Limette", hex: "#a3c94f" }),
  Object.freeze({ name: "Braun", hex: "#b8865b" }),
]);

// Normalisiert einen (ggf. aus localStorage stammenden, potenziell ungültigen) Farbindex
// auf einen gültigen Palette-Index: alles außer einer ganzen Zahl 0..9 fällt auf 0 zurück.
let EXTRA_BOOKMARK_COLORS = [];
const paletteColors = () => BOOKMARK_PALETTE_COLORS.concat(EXTRA_BOOKMARK_COLORS);
function normalizeBookmarkColor(index) {
  return Number.isInteger(index) && index >= 0 && index < paletteColors().length ? index : 0;
}

function bookmarkColorHex(index) {
  return paletteColors()[normalizeBookmarkColor(index)].hex;
}

function bookmarkColorName(index) {
  return paletteColors()[normalizeBookmarkColor(index)].name;
}

const BookmarkPalette = Object.freeze({
  get COLORS() { return paletteColors(); },
  setExtra(colors) { EXTRA_BOOKMARK_COLORS = (colors || []).filter(h => /^#[0-9a-f]{6}$/i.test(h)).slice(0,245).map((hex,i)=>({hex,name:`Farbe ${11+i}`})); },
  normalize: normalizeBookmarkColor,
  hex: bookmarkColorHex,
  name: bookmarkColorName,
});

if (typeof module !== "undefined") module.exports = { BookmarkPalette, BOOKMARK_PALETTE_COLORS, normalizeBookmarkColor, bookmarkColorHex, bookmarkColorName };
