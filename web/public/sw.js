// Service worker de Shiori (lot 6.5a) : lecture en streaming plus rapide et tolérante aux
// coupures, sans rien télécharger d'avance.
// - pages et couvertures : adresses immuables (principe 2), gardées dans un cache borné ;
// - fichiers `assets/` : nommés d'après leur contenu (Vite), gardés tels quels ;
// - navigation : jamais interceptée en ligne (le navigateur garde la main sur la connexion d'un
//   reverse proxy : mot de passe, redirection), la dernière interface reçue servie hors ligne ;
// - le reste de l'API (progression, réglages, listes) : toujours le serveur ; sans réseau, la
//   copie d'un tome téléchargé s'il y en a une (lot 6.5b, `src/offline/downloads.ts`).
// Seules les réponses 200 sont gardées : une page en erreur reste une erreur (principe 4).

const IMAGES = 'shiori-images-v1'
const SHELL = 'shiori-shell-v1'
// Tomes téléchargés à la demande : écrit par l'interface, jamais vidé ici.
const OFFLINE = 'shiori-offline-v1'
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
    event.respondWith(image(request))
  } else if (url.pathname.startsWith('/api/')) {
    event.respondWith(fetch(request).catch((error) => fromOffline(request, error)))
  } else if (url.pathname.startsWith('/assets/')) {
    event.respondWith(cacheFirst(request, SHELL, MAX_ASSETS))
  } else if (request.mode === 'navigate') {
    if (self.navigator.onLine) event.waitUntil(refreshShell())
    else event.respondWith(offlineShell(request))
  }
})

/** Page déjà vue ou téléchargée, sinon le réseau ; sans réseau, la même page dans une autre
 * taille (l'écran a tourné, double page) : toujours la bonne page, jamais une autre. */
async function image(request) {
  // Téléchargement d'un tome (`no-store`) : il garde lui-même ses pages, pas en double ici.
  if (request.cache === 'no-store') return fetch(request)
  const offline = await caches.open(OFFLINE)
  const kept = await offline.match(request)
  if (kept) return kept
  try {
    return await cacheFirst(request, IMAGES, MAX_IMAGES)
  } catch (error) {
    const any = (await offline.match(request, { ignoreSearch: true })) ?? (await caches.match(request, { ignoreSearch: true }))
    if (any) return any
    throw error
  }
}

async function fromOffline(request, error) {
  const kept = await (await caches.open(OFFLINE)).match(request)
  if (kept) return kept
  throw error
}

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
