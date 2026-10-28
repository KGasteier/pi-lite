// Kleine Testarchive, auch absichtlich ungültige Header. Keine System-tar-Aufrufe.
import assert from 'node:assert/strict';
import { gzipSync, gunzipSync } from 'node:zlib';

export function tarGz(entries) {
  const blocks = [];
  for (const { name, data = '', type = '0', link = '', size, mode = 0o644 } of entries) {
    const body = Buffer.from(data);
    const header = Buffer.alloc(512);
    const text = (value, offset, length) => {
      const encoded = Buffer.from(value);
      assert.ok(encoded.length <= length, 'Testwert passt nicht in ustar-Header');
      encoded.copy(header, offset);
    };
    const octal = (value, offset, length) => text(value.toString(8).padStart(length - 1, '0') + '\0', offset, length);
    text(name, 0, 100); octal(mode, 100, 8); octal(0, 108, 8); octal(0, 116, 8);
    octal(size ?? body.length, 124, 12); octal(0, 136, 12);
    header.fill(32, 148, 156); text(type, 156, 1); text(link, 157, 100);
    text('ustar\0', 257, 6); text('00', 263, 2);
    const sum = header.reduce((total, byte) => total + byte, 0);
    text(sum.toString(8).padStart(6, '0') + '\0 ', 148, 8);
    blocks.push(header, body, Buffer.alloc((512 - body.length % 512) % 512));
  }
  return gzipSync(Buffer.concat([...blocks, Buffer.alloc(1024)]));
}

// Nur Test-Auswertung, kein produktiver Entpacker. Nichts wird ins Dateisystem geschrieben.
export function readTarGz(archive) {
  const tar = gunzipSync(archive, { maxOutputLength: 8 * 1024 * 1024 });
  const files = new Map();
  const text = bytes => bytes.toString('utf8').split('\0')[0];
  let offset = 0;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every(byte => byte === 0)) {
      assert.ok(tar.length - offset >= 1024, 'Zwei Abschlussblöcke fehlen');
      assert.ok(tar.subarray(offset).every(byte => byte === 0), 'Daten nach Archivende');
      return files;
    }
    const expected = Number.parseInt(text(header.subarray(148, 156)).trim(), 8);
    const checksum = header.reduce((sum, byte, i) => sum + (i >= 148 && i < 156 ? 32 : byte), 0);
    assert.equal(checksum, expected, 'Tar-Prüfsumme');
    assert.equal(text(header.subarray(257, 263)), 'ustar', 'Kein ustar');
    const prefix = text(header.subarray(345, 500));
    const name = (prefix ? prefix + '/' : '') + text(header.subarray(0, 100));
    assert.ok(!name.startsWith('/') && !name.includes('\\') && !/^[A-Za-z]:/.test(name));
    assert.ok(!name.replace(/\/$/, '').split('/').some(part => !part || part === '.' || part === '..'));
    assert.ok(!files.has(name), 'Doppelter Archivpfad');
    const type = header[156];
    assert.ok(type === 0 || type === 48 || type === 53, 'Link oder Sonderdatei im Archiv');
    const size = Number.parseInt(text(header.subarray(124, 136)).trim(), 8);
    assert.ok(Number.isSafeInteger(size) && size >= 0 && size <= 8 * 1024 * 1024);
    assert.ok(offset + 512 + size <= tar.length, 'Abgeschnittener Tar-Inhalt');
    if (type !== 53) files.set(name, tar.subarray(offset + 512, offset + 512 + size));
    offset += 512 + Math.ceil(size / 512) * 512;
  }
  assert.fail('Archivende fehlt');
}
