// Reine Bilddaten, keine HTML-Fragmente, data-URLs aus Text oder Link-Downloads.
(function (root) {
  const MAX_BYTES = 8 * 1024 * 1024;
  const types = new Set(["image/png", "image/jpeg", "image/webp"]);
  function files(data) {
    const fromItems = Array.from(data?.items || []).filter(i => i.kind === "file" && i.type.startsWith("image/")).map(i => i.getAsFile()).filter(Boolean);
    return fromItems.length ? fromItems : Array.from(data?.files || []).filter(f => f.type.startsWith("image/"));
  }
  async function encode(file) {
    const mime = file.type;
    if (!types.has(mime)) throw new Error("Zwischenablage: PNG, JPEG oder WebP erwartet.");
    if (!file.size || file.size > MAX_BYTES) throw new Error("Zwischenablagebild überschreitet 8 MiB oder ist leer.");
    const b = new Uint8Array(await file.arrayBuffer());
    if (b.length !== file.size || b.length > MAX_BYTES) throw new Error("Ungültige Bildgröße.");
    const starts = (seq, offset = 0) => seq.every((v, i) => b[offset + i] === v);
    const valid = mime === "image/png" ? starts([137,80,78,71,13,10,26,10]) : mime === "image/jpeg" ? starts([255,216,255]) : starts([82,73,70,70]) && starts([87,69,66,80], 8);
    if (!valid) throw new Error("Bildinhalt passt nicht zum Format.");
    let raw = "";
    for (let i = 0; i < b.length; i += 32768) raw += String.fromCharCode(...b.subarray(i, i + 32768));
    return { type: "image", mimeType: mime, data: btoa(raw) };
  }
  const api = { MAX_BYTES, files, encode };
  if (typeof module !== "undefined") module.exports = api;
  else root.ClipboardMaterial = api;
})(globalThis);
