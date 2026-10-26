// Remote-Sessions (Server-Backends über pi-hub) in der gemeinsamen Sessionliste.
// Reine Funktionen, per Node testbar (scripts/test-remote-sessions.cjs).
//
// Schlüssel: lokal der Pfad, remote `remote:<backend>:<pfad>` – identisch zu
// `pi_backend::sessions::remote_key` in copi, damit Bookmarks/Farben in
// `<config>/pi-frontends/bookmarks.json` von beiden Frontends gleich gelesen werden.
// Herkunftsfilter: "all" → "local" → "remote" → "all" (wie copi `session_origin`).
(function (root) {
  "use strict";
  const ORIGINS = ["all", "local", "remote"];
  const ORIGIN_LABELS = { all: "Alle Orte", local: "Lokal", remote: "Remote" };
  const ORIGIN_TITLES = {
    all: "Lokale und Remote-Sessions · Klick: nur lokal",
    local: "Nur lokale Sessions · Klick: nur Remote",
    remote: "Nur Sessions der Server-Backends · Klick: alle Orte",
  };

  function remoteSessionKey(backend, path) { return backend ? `remote:${backend}:${path}` : path; }
  function sessionKey(s) { return s.key || remoteSessionKey(s.backend || "", s.path); }
  function normalizeOrigin(v) { return ORIGINS.includes(v) ? v : "all"; }
  function nextOrigin(v) { return ORIGINS[(ORIGINS.indexOf(normalizeOrigin(v)) + 1) % ORIGINS.length]; }

  // Hub-Liste (`net_sessions`: modified in ms) in das Format von `list_sessions` (Sekunden)
  // bringen und mit Backend-Kennung/Anzeigename versehen.
  function fromHub(backend, label, list) {
    return (list || []).filter(s => s && typeof s.path === "string").map(s => ({
      path: s.path, cwd: s.cwd || "", title: s.title || "", name: "",
      modified: Math.floor((Number(s.modified) || 0) / 1000),
      backend, backendLabel: label || backend, key: remoteSessionKey(backend, s.path),
    }));
  }

  function filterOrigin(list, origin) {
    const o = normalizeOrigin(origin);
    return o === "all" ? [...list] : list.filter(s => (o === "remote") === !!s.backend);
  }

  // Läuft die Remote-Session gerade in einer (nicht beendeten) Instanz des Backends?
  function openInstance(insts, s) {
    const map = (insts || {})[s.backend] || {};
    return Object.values(map).find(i => i.session === s.path && i.state !== "exited") || null;
  }

  const api = { ORIGINS, ORIGIN_LABELS, ORIGIN_TITLES, remoteSessionKey, sessionKey, normalizeOrigin, nextOrigin, fromHub, filterOrigin, openInstance };
  if (typeof module !== "undefined" && module.exports) module.exports = api; else root.RemoteSessions = api;
})(globalThis);
