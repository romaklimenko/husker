// The service worker of the phone page. It keeps the page files, so the page
// opens at once while the server starts. It never handles /api or /mcp: task
// data always comes from the server.
//
// The server writes a version made from all page files into VERSION. A release
// that changes any file therefore changes this script, and the browser installs
// the new worker. That worker fetches every file into a cache of its own before
// it replaces the old one, so files of two releases never mix.

const VERSION = "__PAGE_VERSION__";
const CACHE = `husker-page-${VERSION}`;
const FILES = [
  "/",
  "/app.css",
  "/app.js",
  "/logic.js",
  "/strings.json",
  "/manifest.webmanifest",
  "/icon-192.png",
  "/icon-512.png",
  "/apple-touch-icon.png",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      // "reload" passes the browser's own cache, so every file belongs to this version.
      .then((cache) => cache.addAll(FILES.map((path) => new Request(path, { cache: "reload" }))))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((name) => name !== CACHE).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  const isPageFile = url.origin === self.location.origin && FILES.includes(url.pathname);
  if (event.request.method !== "GET" || !isPageFile) return;
  event.respondWith(
    caches
      .open(CACHE)
      .then((cache) => cache.match(url.pathname))
      .then((kept) => kept ?? fetch(event.request)),
  );
});
