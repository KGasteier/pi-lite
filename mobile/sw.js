// Ausschließlich öffentliche, explizit aufgelistete App-Dateien. Niemals Laufzeit-Caching.
const SCOPE = new URL('./', self.location.href);
const CACHE = `cocopi-mobil-shell-v4-${SCOPE.pathname}`;
const SHELL = [
  './', './index.html', './app.js', './state.js', './workflow.js', './device.js', './style.css',
  './manifest.webmanifest', './icons/cocopi.svg', './icons/cocopi-192.png',
  './icons/cocopi-512.png', './fonts/Figtree-Regular.ttf', '../ui/transport.js',
].map(path => new URL(path, SCOPE).pathname);
self.addEventListener('install', event => {
  event.waitUntil(caches.open(CACHE).then(cache => cache.addAll(SHELL)));
});
self.addEventListener('activate', event => {
  event.waitUntil(caches.keys().then(keys => Promise.all(keys.filter(key => key.startsWith('cocopi-mobil-shell-') && key !== CACHE).map(key => caches.delete(key)))));
});
self.addEventListener('fetch', event => {
  const request = event.request, url = new URL(request.url);
  // Auch ein bekannter Shellpfad mit Header/Query darf keine privaten Antworten speichern.
  if (request.method !== 'GET' || url.origin !== self.location.origin || url.search || url.hash || request.headers.has('Authorization')) return;
  if (!SHELL.includes(url.pathname)) return;
  event.respondWith(caches.open(CACHE).then(cache => cache.match(url.pathname)).then(cached => cached || fetch(request, { cache: 'no-store' })));
});
