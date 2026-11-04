// Airspace service worker: offline app shell.
//
// On install it reads the built index.html and precaches every asset it
// references (hashed JS, WebAssembly, CSS) plus fonts and icons, so the app
// works offline right after the first visit.
//
// It only handles same-origin GET requests for the app's own files. It never
// touches cross-origin requests (AI providers), never caches requests that
// carry authorization headers, and stores no health data (that lives in
// IndexedDB, not the HTTP cache).
const CACHE = "airspace-shell-v2";
const STATIC = ["./", "./index.html", "./manifest.webmanifest", "./fonts/InterVariable.woff2", "./icons/icon.svg", "./icons/icon-192.png", "./icons/icon-512.png"];

async function precache() {
  const cache = await caches.open(CACHE);
  const res = await fetch("./index.html", { cache: "no-store" });
  const html = await res.clone().text();
  await cache.put("./index.html", res);
  const assets = new Set(STATIC);
  // href="./x.css", src=…, and the module loader's './x.js' / './x_bg.wasm'
  for (const m of html.matchAll(/(?:href|src)="(\.\/[^"]+)"/g)) assets.add(m[1]);
  for (const m of html.matchAll(/'(\.\/[^']+\.(?:js|wasm))'/g)) assets.add(m[1]);
  await Promise.all(
    [...assets].map((u) =>
      fetch(u, { cache: "no-store" })
        .then((r) => (r.ok ? cache.put(u, r) : null))
        .catch(() => null)
    )
  );
}

self.addEventListener("install", (event) => {
  event.waitUntil(precache().then(() => self.skipWaiting()));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))).then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== self.location.origin) return; // AI and other network calls bypass the worker
  if (req.headers.has("authorization") || req.headers.has("x-api-key")) return;
  if (req.mode === "navigate") {
    // Network first so updates arrive; fall back to the cached shell offline.
    event.respondWith(
      fetch(req)
        .then((res) => {
          const copy = res.clone();
          caches.open(CACHE).then((c) => c.put("./index.html", copy));
          return res;
        })
        .catch(() => caches.match("./index.html"))
    );
    return;
  }
  // Hashed build assets are immutable: cache first.
  event.respondWith(
    caches.match(req, { ignoreSearch: true }).then(
      (hit) =>
        hit ||
        fetch(req).then((res) => {
          if (res.ok && res.type === "basic") {
            const copy = res.clone();
            caches.open(CACHE).then((c) => c.put(req, copy));
          }
          return res;
        })
    )
  );
});
