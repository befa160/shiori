// Service worker de Shiori (lot 6.5a) : lecture en streaming plus rapide et tolérante aux
// coupures, sans rien télécharger d'avance.
// - pages et couvertures : adresses immuables (principe 2), gardées dans un cache borné ;
// - fichiers `assets/` : nommés d'après leur contenu (Vite), gardés tels quels ;
// - navigation : jamais interceptée en ligne (le navigateur garde la main sur la connexion d'un
//   reverse proxy : mot de passe, redirection), la dernière interface reçue servie hors ligne ;
// - le reste de l'API (progression, réglages, listes) : jamais en cache, toujours le serveur.
// Seules les réponses 200 sont gardées : une page en erreur reste une erreur (principe 4).

const IMAGES = 'shiori-images-v1'
const SHELL = 'shiori-shell-v1'
// shortcut: bornes en nombre d'entrées (pages AVIF de 100 à 500 Ko : 100 à 250 Mo au plus) ;
// passer à une borne en octets si des écrans plus grands le demandent.
const MAX_IMAGES = 500
const MAX_ASSETS = 60

self.addEventListener('install', () => self.skipWaiting())
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()))

self.addEventListener('fetch', (event) => {
  const request = event.request
  if (request.method !== 'GET') return
  const url = new URL(request.url)
  if (url.origin !== self.location.origin) return
  if (/^\/api\/(pages\/[^/]+\/image|covers\/[^/]+)$/.test(url.pathname)) {
    event.respondWith(cacheFirst(request, IMAGES, MAX_IMAGES))
  } else if (url.pathname.startsWith('/assets/')) {
    event.respondWith(cacheFirst(request, SHELL, MAX_ASSETS))
  } else if (request.mode === 'navigate') {
    if (self.navigator.onLine) event.waitUntil(refreshShell())
    else event.respondWith(offlineShell(request))
  }
})

async function cacheFirst(request, name, max) {
  const cache = await caches.open(name)
  const cached = await cache.match(request)
  if (cached) return cached
  const response = await fetch(request)
  if (response.status === 200) {
    await cache.put(request, response.clone())
    await trim(cache, max)
  }
  return response
}

/** Copie de l'interface (`index.html`, la même pour toutes les routes), gardée pour hors ligne. */
async function refreshShell() {
  try {
    const response = await fetch('/', { cache: 'no-cache' })
    if (response.status === 200) await (await caches.open(SHELL)).put('/', response)
  } catch {
    // Réseau coupé entre-temps : la copie précédente reste.
  }
}

async function offlineShell(request) {
  return (await (await caches.open(SHELL)).match('/')) ?? fetch(request)
}

/** Les plus anciennes entrées partent d'abord (ordre d'insertion du cache). */
async function trim(cache, max) {
  const keys = await cache.keys()
  for (const key of keys.slice(0, Math.max(0, keys.length - max))) await cache.delete(key)
}
