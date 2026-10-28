// Nur Testfixture: echter v1-Hub/Sync, Temp-HOME, kein pi/Modell.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import zlib from 'node:zlib';
import { pathToFileURL } from 'node:url';
const repo = process.env.COCOPI_HUB_TEST_DIR;
const { createV1 } = await import(pathToFileURL(path.join(repo, 'lib/hub-v1.mjs')));
const { tarHeader } = await import(pathToFileURL(path.join(repo, 'lib/sync-v1.mjs')));
const root = process.env.HOME;
let hub;
const server = http.createServer((req, res) => {
  const url = new URL(req.url, origin);
  if (url.pathname.startsWith('/v1/sync/archiv/') && req.method === 'GET' && process.env.TEST_BAD_DOWNLOAD) {
    res.writeHead(200, { 'x-cocopi-sha256': process.env.TEST_BAD_DOWNLOAD === 'header' ? '0'.repeat(64) : expectedHash });
    res.end(process.env.TEST_BAD_DOWNLOAD === 'oversize' ? Buffer.alloc(1024*1024) : 'defekt'); return;
  }
  hub.handle(req, res, url);
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;
hub = createV1({ dir: path.join(root, 'state'), slot: 'account01', hubUrl: origin, roots: [root], inRoots: p => p, createInstance() { throw Error('Keine Agenten im Sync-Test'); }, empfangPubkey: undefined });
const invite = hub.devices.issue('e', { person: 'Test' });
const bytes = Buffer.from('Persönliches Ergebnis\n');
const tar = Buffer.concat([tarHeader('text.md', bytes.length), bytes, Buffer.alloc((512 - bytes.length % 512) % 512), Buffer.alloc(1024)]);
const data = zlib.gzipSync(tar);
fs.mkdirSync(path.join(root, 'Ergebnisse'), { recursive: true });
fs.writeFileSync(path.join(root, 'Ergebnisse', 'neu.md'), 'Rückläufer');
let expectedHash;
const originalEnd = http.ServerResponse.prototype.end;
// Nur die Test-Metadaten für absichtlich beschädigten Download merken.
http.ServerResponse.prototype.end = function(chunk, ...args) {
  if (typeof chunk === 'string') { try { const v = JSON.parse(chunk); if (v.typ === 'archiv_bereit') expectedHash = v.sha256; } catch {} }
  return originalEnd.call(this, chunk, ...args);
};
console.log(JSON.stringify({ origin, code: invite.code, upload: data.toString('hex') }));
process.stdin.resume(); process.stdin.on('end', () => { hub.close(); server.closeAllConnections(); server.close(); });
