// Pure input-history state; also usable in Node tests.
(function (root) {
  class InputHistory {
    constructor() { this.items = []; this.reset(); }
    reset() { this.index = this.items.length; this.draft = ""; }
    browsing() { return this.index < this.items.length; }
    replace(items) { this.items = items; this.reset(); }
    static text(message) {
      if (message?.role !== "user") return null;
      const c = message.content;
      const text = typeof c === "string" ? c : Array.isArray(c) ? c.filter(p => p?.type === "text" && typeof p.text === "string").map(p => p.text).join("\n") : "";
      return text.trim() ? text : null;
    }
    static entries(entries) { return Array.isArray(entries) ? entries.filter(e => e?.type === "message").map(e => this.text(e.message)).filter(t => t !== null) : []; }
    move(direction, current) {
      if (!this.items.length) return current;
      if (this.index === this.items.length) this.draft = current;
      this.index = Math.max(0, Math.min(this.items.length, this.index + direction));
      return this.index === this.items.length ? this.draft : this.items[this.index];
    }
  }
  if (typeof module !== "undefined") module.exports = InputHistory;
  else root.InputHistory = InputHistory;
})(globalThis);
