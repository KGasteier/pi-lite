// Local UI metadata only; session files remain untouched.
//
// Bookmark-Filter/Sortierung: ein einziger Drei-Zustands-Zyklus statt getrennter
// Filter-/Sortierauswahl (ersetzt die frühere Kombination aus Bookmark-Filter-Toggle
// und separatem Sortier-Select):
//   "off"    – alle Sessions, rein nach Zeit (neueste Änderung zuerst).
//   "on"     – nur Bookmarks, ebenfalls nach Zeit.
//   "colors" – nur Bookmarks, primär nach Bookmark-Farbindex (Palettenreihenfolge),
//              innerhalb gleicher Farbe nach letzter Nutzung (Zeit).
// Reihenfolge des Zyklus: off -> on -> colors -> off. Zustand ist lokal persistent.
//
// Bookmark-Farben: `colors` speichert einen Palette-Index (0..=7, siehe
// `ui/bookmark-palette.js` für die acht festen Hex-Codes/Namen) je markiertem Pfad. Diese
// Datei kennt bewusst keine Hex-Werte/Namen selbst (Trennung Persistenz/Darstellung) – nur
// die Palettengröße (8), um ungültige Indizes (fremd editierter localStorage-Inhalt,
// künftig verkleinerte Palette) sicher auf 0 (Vorgabefarbe) zurückzufallen, statt eine
// ungültige Farbe zu persistieren. Index 0 wird nie explizit gespeichert (spart Platz,
// entspricht dem Verhalten vor Einführung der Farbauswahl: ein Bookmark ohne eigenen
// Eintrag ist die Vorgabefarbe).
const PALETTE_SIZE = 8;
const FILTER_MODES = ["off", "on", "colors"];
function normalizeColorIndex(index) {
  return Number.isInteger(index) && index >= 0 && index < PALETTE_SIZE ? index : 0;
}
function normalizeFilterMode(value) {
  return FILTER_MODES.includes(value) ? value : "off";
}

function createSessionBookmarks(storage) {
  const key = "pi-lite.session-bookmarks.v1", filterKey = "pi-lite.session-filter-mode.v1",
    colorKey = "pi-lite.session-bookmark-colors.v1";
  let paths = new Set(), filterMode = "off", colors = new Map();
  try {
    const saved = JSON.parse(storage.getItem(key) || "[]");
    if (Array.isArray(saved)) paths = new Set(saved.filter(p => typeof p === "string"));
    // Migration von der alten Boolean-Filterablage ("all"/"bookmarks") auf den
    // Drei-Zustands-Modus: "bookmarks" wird zu "on", alles andere bleibt "off".
    const storedFilter = storage.getItem(filterKey);
    filterMode = storedFilter === "bookmarks" ? "on" : normalizeFilterMode(storedFilter);
    const savedColors = JSON.parse(storage.getItem(colorKey) || "{}");
    if (savedColors && typeof savedColors === "object" && !Array.isArray(savedColors)) {
      for (const [p, idx] of Object.entries(savedColors)) {
        // Nur Farben für tatsächlich markierte Pfade übernehmen (kein verwaister Zustand,
        // falls ein Bookmark z. B. durch eine ältere App-Version zwischenzeitlich entfernt
        // wurde, ohne seine Farbe aufzuräumen).
        const n = normalizeColorIndex(idx);
        if (typeof p === "string" && paths.has(p) && n !== 0) colors.set(p, n);
      }
    }
  } catch (_) { /* Corrupt/unavailable storage must not break sessions. */ }
  const saveColors = () => storage.setItem(colorKey, JSON.stringify(Object.fromEntries(colors)));
  return {
    has: path => paths.has(path),
    get filterMode() { return filterMode; },
    get onlyBookmarks() { return filterMode !== "off"; }, // Kompatibilitätsgetter für einfache Abfragen
    setFilterMode(value) {
      const next = normalizeFilterMode(value);
      storage.setItem(filterKey, next); filterMode = next;
      return filterMode;
    },
    cycleFilterMode() {
      const next = FILTER_MODES[(FILTER_MODES.indexOf(filterMode) + 1) % FILTER_MODES.length];
      return this.setFilterMode(next);
    },
    visible: sessions => filterMode === "off" ? [...sessions] : sessions.filter(s => paths.has(s.path)),
    toggle(path) {
      const next = new Set(paths);
      if (next.has(path)) {
        next.delete(path);
        // Farbe einer entfernten Markierung aufräumen statt verwaist stehen zu lassen.
        if (colors.has(path)) { colors.delete(path); saveColors(); }
      } else {
        next.add(path);
      }
      storage.setItem(key, JSON.stringify([...next])); paths = next;
      return paths.has(path);
    },
    // Sortierung ergibt sich direkt aus dem Filtermodus (kein separates Sortier-Select
    // mehr nötig): "colors" sortiert primär nach Palettenindex, sonst rein nach Zeit.
    sort(sessions) {
      return [...sessions].sort((a, b) =>
        (filterMode === "colors" ? this.colorOf(a.path) - this.colorOf(b.path) : 0) ||
        b.modified - a.modified || a.path.localeCompare(b.path));
    },
    // Farbindex eines Bookmarks (0..=7, normalisiert); 0 (Vorgabe) auch für nicht markierte
    // Pfade oder Pfade ohne eigenen Eintrag.
    colorOf(path) {
      return colors.has(path) ? colors.get(path) : 0;
    },
    // Setzt die Bookmark-Farbe eines **markierten** Pfads; ohne Wirkung, falls `path` kein
    // Bookmark ist (keine verwaisten Farbeinträge für nicht markierte Sessions).
    setColor(path, index) {
      if (!paths.has(path)) return false;
      const n = normalizeColorIndex(index);
      if (n === 0) colors.delete(path); else colors.set(path, n);
      saveColors();
      return true;
    }
  };
}
if (typeof module !== "undefined") module.exports = { createSessionBookmarks, normalizeColorIndex, normalizeFilterMode, PALETTE_SIZE, FILTER_MODES };
