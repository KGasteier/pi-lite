// Acht feste Bookmark-Standardfarben für Session-Lesezeichen (pi-lite).
//
// Getrennt von session-bookmarks.js, weil diese Palette ausschließlich Hex-Codes/Namen
// definiert und von mehreren Stellen gebraucht wird (Sidebar-Karte, Kontextmenü,
// Farbauswahl-Button). Die Werte sind bewusst 1:1 identisch zu
// copi/app/src/bookmark_palette.rs (gleiche Hex-Codes, gleiche Reihenfolge, gleiche
// deutsche Namen). index.html lädt die Palette vor session-bookmarks.js; app.js nutzt
// sie für die Session-Karten und den Farbwähler.
//
// Index 0 ist die Standardfarbe neuer Bookmarks (entspricht der bisherigen, einfarbigen
// Kategoriefarbe --cat), Indizes 1..=7 sind frei wählbare Zusatzfarben. Ein ungültiger
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
]);

// Normalisiert einen (ggf. aus localStorage stammenden, potenziell ungültigen) Farbindex
// auf einen gültigen Palette-Index: alles außer einer ganzen Zahl 0..7 fällt auf 0 zurück.
function normalizeBookmarkColor(index) {
  return Number.isInteger(index) && index >= 0 && index < BOOKMARK_PALETTE_COLORS.length ? index : 0;
}

function bookmarkColorHex(index) {
  return BOOKMARK_PALETTE_COLORS[normalizeBookmarkColor(index)].hex;
}

function bookmarkColorName(index) {
  return BOOKMARK_PALETTE_COLORS[normalizeBookmarkColor(index)].name;
}

const BookmarkPalette = Object.freeze({
  COLORS: BOOKMARK_PALETTE_COLORS,
  normalize: normalizeBookmarkColor,
  hex: bookmarkColorHex,
  name: bookmarkColorName,
});

if (typeof module !== "undefined") module.exports = { BookmarkPalette, BOOKMARK_PALETTE_COLORS, normalizeBookmarkColor, bookmarkColorHex, bookmarkColorName };
