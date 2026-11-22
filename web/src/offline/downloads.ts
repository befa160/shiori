// Tomes téléchargés pour lire hors ligne (lot 6.5b), sur demande seulement. Tout passe par le
// cache `OFFLINE_CACHE`, que le service worker (`public/sw.js`) sert quand le réseau manque et ne
// vide jamais seul : les réponses JSON que lit le lecteur, puis chaque page à la taille qu'il
// demanderait sur cet écran, en portrait. Page par page : aucune requête au-delà de quelques Mo
// (reverse proxy et Cloudflare limitent la taille d'une requête), jamais une archive entière.
import { useSyncExternalStore } from 'react'
import { toast } from 'sonner'
import { ApiError } from '@/api/client'
import type { PageInfo } from '@/api/types'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'
import { pageUrl } from '@/reader/controls'

/** Même nom que dans `public/sw.js`. */
export const OFFLINE_CACHE = 'shiori-offline-v1'
const INDEX = 'shiori.downloads'
/** File d'attente, gardée pour reprendre après une fermeture de l'application. */
const QUEUE = 'shiori.downloadQueue'
/** Pages demandées en même temps : assez pour remplir la ligne, sans accaparer le serveur. */
const PARALLEL = 3

export interface Download {
  volumeId: string
  seriesId: number
  series: string
  label: string
  pages: number
  bytes: number
}

export type Request = Pick<Download, 'volumeId' | 'seriesId' | 'series' | 'label'>

export interface Job extends Request {
  done: number
  total: number
}

interface State {
  downloads: Download[]
  /** File d'attente ; la première est en cours. */
  queue: Job[]
}

let state: State = { downloads: readIndex(), queue: [] }
let running: AbortController | null = null
/** Change à chaque remise à zéro de test : une ancienne boucle de téléchargement s'arrête. */
let generation = 0
const listeners = new Set<() => void>()

function readList<T>(key: string): T[] {
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? '[]')
    return Array.isArray(saved) ? saved : []
  } catch {
    return []
  }
}

function readIndex(): Download[] {
  return readList(INDEX)
}

function save(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // Stockage refusé : les tomes restent dans le cache, seule la liste sera perdue.
  }
}

function update(next: Partial<State>) {
  state = { ...state, ...next }
  if (next.downloads) save(INDEX, state.downloads)
  if (next.queue) save(QUEUE, state.queue.map(({ done: _d, total: _t, ...request }) => request))
  for (const listener of listeners) listener()
}

/** Au lancement : efface ce qu'un téléchargement interrompu (application fermée) a laissé, puis
 * reprend la file là où elle en était, au retour du réseau s'il manque (sinon chaque tome
 * échouerait aussitôt et sortirait de la file). */
export async function resumeDownloads() {
  const known = new Set(state.downloads.map((d) => d.volumeId))
  const cache = await caches.open(OFFLINE_CACHE)
  for (const key of await cache.keys()) {
    const volume = /^\/api\/volumes\/([^/]+)\/pages$/.exec(new URL(key.url).pathname)?.[1]
    if (volume && !known.has(volume)) await deleteVolume(volume)
  }
  const resume = () => enqueue(readList<Request>(QUEUE))
  if (navigator.onLine) resume()
  else addEventListener('online', resume, { once: true })
}

/** Un tome téléchargé ne se lit sans réseau que si le service worker contrôle la page : lui seul
 * sert le cache hors ligne. Sans lui (PWA installée depuis un autre navigateur ou à une autre
 * adresse, version de développement), le téléchargement réussirait pour rien. */
export function offlineReady(): boolean {
  return 'caches' in window && Boolean(navigator.serviceWorker?.controller)
}

function onControllerChange(onChange: () => void) {
  navigator.serviceWorker?.addEventListener('controllerchange', onChange)
  return () => navigator.serviceWorker?.removeEventListener('controllerchange', onChange)
}

/** `offlineReady`, suivi en direct (le service worker prend la main après l'installation). */
export function useOfflineReady(): boolean {
  return useSyncExternalStore(onControllerChange, offlineReady)
}

/** Tomes téléchargés et file d'attente, suivis en direct. */
export function useOffline(): State {
  return useSyncExternalStore(
    (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    () => state,
  )
}

/** Ajoute des tomes à la file (ceux déjà téléchargés ou en attente sont ignorés). */
export function enqueue(requests: Request[]) {
  const known = new Set([...state.downloads, ...state.queue].map((d) => d.volumeId))
  const fresh = requests.filter((r) => !known.has(r.volumeId)).map((r) => ({ ...r, done: 0, total: 0 }))
  if (fresh.length === 0) return
  if (!offlineReady()) {
    toast.error(i18n.t('offline.unavailable'), { description: i18n.t('offline.unavailableHelp') })
    return
  }
  update({ queue: [...state.queue, ...fresh] })
  // Le navigateur pourrait sinon vider le stockage de lui-même quand la place manque.
  void navigator.storage?.persist?.()
  // Le code du lecteur se charge à la demande : chargé maintenant, le service worker le garde, et
  // le tome s'ouvrira hors ligne même si le lecteur n'a jamais servi avec cette version.
  void import('@/reader/Reader')
  if (!running) void run()
}

/** Retire un tome de la file, ou arrête son téléchargement en cours (rien n'en reste). */
export function cancel(volumeId: string) {
  if (state.queue[0]?.volumeId === volumeId && running) running.abort()
  else update({ queue: state.queue.filter((j) => j.volumeId !== volumeId) })
}

export async function remove(volumeId: string) {
  await deleteVolume(volumeId)
  update({ downloads: state.downloads.filter((d) => d.volumeId !== volumeId) })
}

async function run() {
  const mine = generation
  while (state.queue.length > 0 && mine === generation) {
    const job = state.queue[0]
    running = new AbortController()
    try {
      const { pages, bytes } = await download(job, running.signal)
      const { done: _done, total: _total, ...request } = job
      update({ downloads: [...state.downloads, { ...request, pages, bytes }] })
    } catch (error) {
      await deleteVolume(job.volumeId)
      if (!running.signal.aborted) {
        toast.error(i18n.t('offline.failed', { label: job.label }), { description: errorText(error as Error, i18n.t) })
      }
    }
    if (mine !== generation) return
    running = null
    update({ queue: state.queue.filter((j) => j.volumeId !== job.volumeId) })
  }
}

/** Écran en portrait (le lecteur demande les plus grandes pages dans ce sens). */
function portrait() {
  const [width, height] = [screen.width, screen.height].sort((a, b) => a - b)
  return { width, height }
}

async function download(job: Job, signal: AbortSignal) {
  const cache = await caches.open(OFFLINE_CACHE)
  const keep = async (url: string) => {
    // `no-store` : le service worker ne la garde pas une seconde fois dans le cache de lecture.
    const response = await fetch(url, { signal, cache: 'no-store' }).catch((error: Error) => {
      if (signal.aborted) throw error
      throw new ApiError(0, 'network', 'serveur injoignable')
    })
    // Une page qui échoue fait échouer le tome : jamais un tome à moitié présenté comme complet.
    if (!response.ok) {
      const body = await response.json().catch(() => ({}))
      throw new ApiError(response.status, body.code ?? 'internal', body.message ?? `HTTP ${response.status}`)
    }
    await cache.put(url, response.clone())
    return response
  }
  for (const url of ['/api/settings', `/api/series/${job.seriesId}`, `/api/volumes/${job.volumeId}`]) await keep(url)
  const pages: PageInfo[] = await (await keep(`/api/volumes/${job.volumeId}/pages`)).json()
  // Une page cassée n'a rien à télécharger : le lecteur l'affichera en erreur, comme en ligne.
  const readable = pages.filter((p) => p.status === 'ok')
  const view = portrait()
  let next = 0
  let bytes = 0
  const progress = (done: number) =>
    update({ queue: state.queue.map((j) => (j.volumeId === job.volumeId ? { ...j, done, total: readable.length } : j)) })
  progress(0)
  let done = 0
  const worker = async () => {
    while (next < readable.length) {
      const response = await keep(pageUrl(readable[next++], view))
      // Taille lue avant l'ajout : `bytes += await …` lirait `bytes` avant l'attente et perdrait
      // les pages finies entre-temps par les autres requêtes.
      const size = (await response.blob()).size
      bytes += size
      progress(++done)
    }
  }
  await Promise.all(Array.from({ length: PARALLEL }, worker))
  return { pages: readable.length, bytes }
}

/** Efface les pages d'un tome et ses fiches ; celle de la série part avec son dernier tome. */
async function deleteVolume(volumeId: string) {
  const cache = await caches.open(OFFLINE_CACHE)
  const listing = await cache.match(`/api/volumes/${volumeId}/pages`)
  const ids = new Set(listing ? ((await listing.json()) as PageInfo[]).map((p) => p.id) : [])
  for (const key of await cache.keys()) {
    const page = /^\/api\/pages\/([^/]+)\/image$/.exec(new URL(key.url).pathname)?.[1]
    if (page && ids.has(decodeURIComponent(page))) await cache.delete(key)
  }
  await cache.delete(`/api/volumes/${volumeId}/pages`)
  await cache.delete(`/api/volumes/${volumeId}`)
  const seriesId = state.downloads.find((d) => d.volumeId === volumeId)?.seriesId
  const others = state.downloads.some((d) => d.seriesId === seriesId && d.volumeId !== volumeId)
  if (seriesId !== undefined && !others) await cache.delete(`/api/series/${seriesId}`)
}

/** Pour les tests : repart d'un état vide. */
export function resetForTests() {
  generation++
  running?.abort()
  running = null
  state = { downloads: readIndex(), queue: [] }
  listeners.clear()
}
