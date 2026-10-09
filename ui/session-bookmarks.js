// Local UI metadata only; session files remain untouched.
function createSessionBookmarks(storage) {
  const key = "pi-lite.session-bookmarks.v1", sortKey = "pi-lite.session-sort.v1", filterKey = "pi-lite.session-filter.v1";
  let paths = new Set(), mode = "time", onlyBookmarks = false;
  try {
    const saved = JSON.parse(storage.getItem(key) || "[]");
    if (Array.isArray(saved)) paths = new Set(saved.filter(p => typeof p === "string"));
    if (storage.getItem(sortKey) === "bookmarks") mode = "bookmarks";
    onlyBookmarks = storage.getItem(filterKey) === "bookmarks";
  } catch (_) { /* Corrupt/unavailable storage must not break sessions. */ }
  return {
    has: path => paths.has(path),
    get mode() { return mode; },
    get onlyBookmarks() { return onlyBookmarks; },
    setOnlyBookmarks(value) { storage.setItem(filterKey, value ? "bookmarks" : "all"); onlyBookmarks = !!value; },
    visible: sessions => onlyBookmarks ? sessions.filter(s => paths.has(s.path)) : [...sessions],
    toggle(path) {
      const next = new Set(paths);
      if (next.has(path)) next.delete(path); else next.add(path);
      storage.setItem(key, JSON.stringify([...next])); paths = next;
      return paths.has(path);
    },
    setMode(value) {
      const next = value === "bookmarks" ? "bookmarks" : "time";
      storage.setItem(sortKey, next); mode = next;
    },
    sort(sessions) {
      return [...sessions].sort((a,b) =>
        (mode === "bookmarks" ? Number(paths.has(b.path))-Number(paths.has(a.path)) : 0) ||
        b.modified-a.modified || a.path.localeCompare(b.path));
    }
  };
}
