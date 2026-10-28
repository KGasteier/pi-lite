// Ausschließlich öffentliche, explizit aufgelistete App-Dateien. Niemals Laufzeit-Caching.
const CACHE = 'cocopi-mobil-shell-v3';
const SHELL = [
  '/m/', '/m/index.html', '/m/app.js', '/m/state.js', '/m/workflow.js', '/m/device.js', '/m/style.css',
  '/m/manifest.webmanifest', '/m/icons/cocopi.svg', '/m/icons/cocopi-192.png',
  '/m/icons/cocopi-512.png', '/m/fonts/Figtree-Regular.ttf', '/ui/transport.js',
];
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
