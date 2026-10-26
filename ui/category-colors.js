// Editierbare, persistente Namen für die zehn globalen Sidebar-Kategoriefarben (pi-lite).
//
// Getrennt von bookmark-palette.js: die zehn Hex-Codes dort sind fix (Parität mit
// copi/app/src/bookmark_palette.rs), aber ihre *Namen* dürfen hier vom Nutzer umbenannt
// werden (z. B. "Blau" -> "Dringend"), ohne die Hex-Werte selbst zu ändern. Das betrifft
// sowohl die Bookmark-Farbwahl als auch jede Stelle, die einen Farbnamen anzeigt (Label im
// Farbwähler). Persistenz ist rein lokal (localStorage), kein Sync, kein Export.
const CATEGORY_COLOR_NAMES_KEY = "pi-lite.category-color-names.v1";

function createCategoryColorNames(storage, defaults) {
  const names = [...defaults];
  const reload = () => {
  names.splice(0, names.length, ...defaults);
  let saved = {};
  try {
    const parsed = JSON.parse(storage.getItem(CATEGORY_COLOR_NAMES_KEY) || "{}");
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) saved = parsed;
  } catch (_) { saved = {}; /* beschädigter Speicher darf die Vorgabenamen nicht zerstören */ }
  for (const [k, v] of Object.entries(saved)) {
    const i = Number(k);
    if (Number.isInteger(i) && i >= 0 && i < names.length && typeof v === "string" && v.trim()) names[i] = v.trim();
  }
  };
  reload();
  const persist = () => {
    const overrides = {};
    names.forEach((n, i) => { if (n !== defaults[i]) overrides[i] = n; });
    try { storage.setItem(CATEGORY_COLOR_NAMES_KEY, JSON.stringify(overrides)); } catch (_) { /* best effort */ }
  };
  return {
    nameOf(index) { reload(); return Number.isInteger(index) && index >= 0 && index < names.length ? names[index] : defaults[0]; },
    all() { reload(); return [...names]; },
    // Leerer/nur-Leerzeichen-Name setzt auf die Vorgabe zurück statt einen leeren Namen
    // zu speichern (keine unsichtbaren Labels in der UI).
    rename(index, value) {
      reload();
      if (!Number.isInteger(index) || index < 0 || index >= names.length) return false;
      const trimmed = (value || "").trim();
      names[index] = trimmed || defaults[index];
      persist();
      return true;
    },
    reset(index) {
      reload();
      if (!Number.isInteger(index) || index < 0 || index >= names.length) return false;
      names[index] = defaults[index];
      persist();
      return true;
    },
  };
}
if (typeof module !== "undefined") module.exports = { createCategoryColorNames, CATEGORY_COLOR_NAMES_KEY };
