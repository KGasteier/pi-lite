// Deliberately conservative: inline file references, never arbitrary prose or URLs.
function editorReference(value) {
  if (typeof value !== "string") return null;
  let path = value.trim(), line = null, column = null;
  if (path.startsWith("file://")) {
    try { const url = new URL(path); if (url.hostname && url.hostname !== "localhost") return null; path = decodeURIComponent(url.pathname) + url.hash; } catch { return null; }
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(path) || /^(?:mailto|javascript|data|vscode|command):/i.test(path) || /[\n\r\0]/.test(path)) return null;
  const location = path.match(/(?::(\d+)(?::(\d+))?|#L(\d+)(?:-L?\d+)?)$/);
  if (location) {
    line = Number(location[1] || location[3]); column = Number(location[2] || 1);
    path = path.slice(0, location.index);
  }
  if (!/(?:^|\/)[^/]+\.[a-z0-9_-]{1,12}$/i.test(path) || /[<>"`|?*]/.test(path)) return null;
  return { path, line, column };
}
if (typeof module !== "undefined") module.exports = { editorReference };
