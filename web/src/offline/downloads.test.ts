import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageInfo } from '@/api/types'
import { fakeCaches } from '@/test/caches'
import { json } from '@/test/render'
import { OFFLINE_CACHE, cancel, enqueue, remove, resetForTests, resumeDownloads, useOffline, useOfflineReady } from './downloads'

const toast = vi.hoisted(() => ({ error: vi.fn() }))
vi.mock('sonner', () => ({ toast }))

const page = (id: string, status: PageInfo['status'] = 'ok'): PageInfo => ({ id, ordinal: 0, width: 1200, height: 1800, media_type: 'image/jpeg', status })
const PAGES = [page('p1'), page('p2', 'broken'), page('p3')]
const request = (volumeId: string, seriesId = 7) => ({ volumeId, seriesId, series: 'Toriko', label: `Tome ${volumeId}` })

let kept: ReturnType<typeof fakeCaches>
/** Le serveur : fiches JSON, puis chaque page (4 octets) ; `fail` répond en erreur à une adresse. */
function serve({ fail, hang }: { fail?: string; hang?: string } = {}) {
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === hang) {
      return new Promise<Response>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('arrêt', 'AbortError'))))
    }
    if (fail && url.startsWith(fail)) return json({ code: 'internal', message: 'boum' }, 500)
    if (url.startsWith('/api/pages/')) return new Response(new Uint8Array(4))
    if (url.endsWith('/pages')) return json(PAGES)
    return json({ url })
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

const state = () => renderHook(() => useOffline()).result

beforeEach(() => {
  localStorage.clear()
  kept = fakeCaches()
  vi.stubGlobal('screen', { width: 800, height: 1280 })
  resetForTests()
})
afterEach(() => {
  vi.unstubAllGlobals()
  toast.error.mockClear()
})

describe('tomes téléchargés', () => {
  it('garde les fiches du lecteur et chaque page lisible, à la taille de l’écran en portrait', async () => {
    const fetch = serve()
    const result = state()
    act(() => enqueue([request('a')]))
    await waitFor(() => expect(result.current.queue).toEqual([]))
    expect(result.current.downloads).toEqual([{ ...request('a'), pages: 2, bytes: 8 }])
    expect(JSON.parse(localStorage.getItem('shiori.downloads')!)).toEqual(result.current.downloads)
    expect(kept(OFFLINE_CACHE).sort()).toEqual(
      ['/api/pages/p1/image?w=800&f=avif', '/api/pages/p3/image?w=800&f=avif', '/api/series/7', '/api/settings', '/api/volumes/a', '/api/volumes/a/pages'].sort(),
    )
    // Jamais en double dans le cache de lecture du service worker.
    expect(fetch).toHaveBeenCalledWith('/api/pages/p1/image?w=800&f=avif', expect.objectContaining({ cache: 'no-store' }))
    // Déjà téléchargé : rien de plus.
    act(() => enqueue([request('a')]))
    expect(result.current.queue).toEqual([])
  })

  it('sans service worker qui contrôle la page, refuse de télécharger et le dit', async () => {
    const fetch = serve()
    let changed = () => {}
    const worker = { controller: null as object | null, addEventListener: (_: string, f: () => void) => (changed = f), removeEventListener() {} }
    Object.defineProperty(navigator, 'serviceWorker', { configurable: true, value: worker })
    const result = state()
    const ready = renderHook(() => useOfflineReady()).result
    expect(ready.current).toBe(false)
    act(() => enqueue([request('a')]))
    expect(result.current.queue).toEqual([])
    expect(fetch).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalledWith('Lecture hors ligne indisponible ici', { description: expect.stringMatching(/service worker/) })
    // Le service worker prend la main (après l'installation) : l'interface le voit aussitôt.
    worker.controller = {}
    act(() => changed())
    expect(ready.current).toBe(true)
  })

  it('suit l’avancement page par page, un tome après l’autre', async () => {
    serve()
    const result = state()
    act(() => enqueue([request('a'), request('b')]))
    expect(result.current.queue.map((j) => j.volumeId)).toEqual(['a', 'b'])
    await waitFor(() => expect(result.current.downloads.map((d) => d.volumeId)).toEqual(['a', 'b']))
  })

  it('une page en erreur interrompt le tome : rien n’en reste, et on le dit', async () => {
    serve({ fail: '/api/pages/p3/' })
    const result = state()
    act(() => enqueue([request('a')]))
    await waitFor(() => expect(result.current.queue).toEqual([]))
    expect(result.current.downloads).toEqual([])
    expect(kept(OFFLINE_CACHE).filter((url) => url.startsWith('/api/pages/'))).toEqual([])
    expect(toast.error).toHaveBeenCalledWith('Téléchargement de Tome a interrompu', { description: 'Erreur interne du serveur' })
  })

  it('serveur injoignable : erreur réseau traduite', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))))
    const result = state()
    act(() => enqueue([request('a')]))
    await waitFor(() => expect(toast.error).toHaveBeenCalled())
    expect(toast.error.mock.calls[0][1].description).not.toContain('Failed to fetch')
    expect(result.current.downloads).toEqual([])
  })

  it('s’annule en cours (sans message) ou dans la file', async () => {
    serve({ hang: '/api/pages/p1/image?w=800&f=avif' })
    const result = state()
    act(() => enqueue([request('a'), request('b')]))
    act(() => cancel('b'))
    expect(result.current.queue.map((j) => j.volumeId)).toEqual(['a'])
    await waitFor(() => expect(result.current.queue[0].total).toBe(2))
    act(() => cancel('a'))
    await waitFor(() => expect(result.current.queue).toEqual([]))
    expect(result.current.downloads).toEqual([])
    expect(toast.error).not.toHaveBeenCalled()
    expect(kept(OFFLINE_CACHE).filter((url) => url.includes('/volumes/a'))).toEqual([])
  })

  it('supprime un tome ; la fiche de la série part avec son dernier tome', async () => {
    serve()
    const result = state()
    act(() => enqueue([request('a'), request('b')]))
    await waitFor(() => expect(result.current.downloads).toHaveLength(2))
    // Les deux tomes partagent les mêmes pages ici : la première suppression les retire toutes.
    await act(() => remove('a'))
    expect(kept(OFFLINE_CACHE)).toContain('/api/series/7')
    expect(kept(OFFLINE_CACHE)).not.toContain('/api/volumes/a')
    await act(() => remove('b'))
    expect(kept(OFFLINE_CACHE).sort()).toEqual(['/api/settings'])
    expect(result.current.downloads).toEqual([])
  })

  it('une liste illisible ou un stockage refusé ne casse rien', async () => {
    localStorage.setItem('shiori.downloads', '{pas du json')
    resetForTests()
    expect(state().current.downloads).toEqual([])
    localStorage.setItem('shiori.downloads', '{"a":1}')
    resetForTests()
    expect(state().current.downloads).toEqual([])
    serve()
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('plein')
    })
    const result = state()
    act(() => enqueue([request('a')]))
    await waitFor(() => expect(result.current.downloads).toHaveLength(1))
    vi.restoreAllMocks()
  })

  it('après une fermeture, efface le tome interrompu et reprend la file', async () => {
    serve({ hang: '/api/pages/p1/image?w=800&f=avif' })
    act(() => enqueue([request('a'), request('b')]))
    await waitFor(() => expect(kept(OFFLINE_CACHE)).toContain('/api/volumes/a/pages'))
    expect(JSON.parse(localStorage.getItem('shiori.downloadQueue')!).map((r: { volumeId: string }) => r.volumeId)).toEqual(['a', 'b'])
    // L'application est fermée en plein téléchargement, puis relancée.
    resetForTests()
    serve()
    const result = state()
    await act(() => resumeDownloads())
    await waitFor(() => expect(result.current.downloads.map((d) => d.volumeId)).toEqual(['a', 'b']))
    expect(localStorage.getItem('shiori.downloadQueue')).toBe('[]')
  })

  it('lancée sans réseau, la reprise attend le retour du réseau', async () => {
    localStorage.setItem('shiori.downloadQueue', JSON.stringify([request('a')]))
    resetForTests()
    serve()
    const online = vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(false)
    const result = state()
    await act(() => resumeDownloads())
    expect(result.current.queue).toEqual([])
    online.mockReturnValue(true)
    act(() => void dispatchEvent(new Event('online')))
    await waitFor(() => expect(result.current.downloads.map((d) => d.volumeId)).toEqual(['a']))
    expect(toast.error).not.toHaveBeenCalled()
    vi.restoreAllMocks()
  })
})
