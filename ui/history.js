// Pure input-history state; also usable in Node tests.
(function (root) {
  class InputHistory {
    constructor(limit = 100) { this.items = []; this.limit = limit; this.reset(); }
    reset() { this.index = this.items.length; this.draft = ""; }
    add(text) {
      if (text && this.items.at(-1) !== text) this.items.push(text);
      this.items = this.items.slice(-this.limit); this.reset();
    }
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
