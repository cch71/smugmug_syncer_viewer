// Service worker for the PWA build.
//
// - App shell (HTML, wasm, JS, CSS): network first, falling back to the cache when offline.
// - Archive media (anything under /artifacts/): cache first, so photos already viewed work
//   offline.  The archive's metadata (.smugmug_db) is always fetched fresh when online.
const SHELL_CACHE = 'shell-v1';
const MEDIA_CACHE = 'media-v1';

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(SHELL_CACHE).then((cache) => cache.addAll(['./', './manifest.webmanifest'])));
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  const keep = [SHELL_CACHE, MEDIA_CACHE];
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((k) => !keep.includes(k)).map((k) => caches.delete(k))))
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET' || request.headers.has('range')) return;
  const url = new URL(request.url);

  if (url.pathname.includes('/artifacts/')) {
    event.respondWith(
      caches.open(MEDIA_CACHE).then(async (cache) => {
        const hit = await cache.match(request);
        if (hit) return hit;
        const response = await fetch(request);
        if (response.ok) cache.put(request, response.clone());
        return response;
      })
    );
    return;
  }

  if (url.origin === self.location.origin && !url.pathname.includes('/.smugmug_db/')) {
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (response.ok) caches.open(SHELL_CACHE).then((cache) => cache.put(request, response.clone()));
          return response;
        })
        .catch(() => caches.match(request).then((hit) => hit || caches.match('./')))
    );
  }
});
