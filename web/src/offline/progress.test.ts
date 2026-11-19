import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/client'
import { json } from '@/test/render'
import { flushProgress, pendingPage, queueProgress, retryable, setProgressUser } from './progress'

beforeEach(() => {
  localStorage.clear()
  localStorage.setItem('shiori.user', '1')
})
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('progression lue hors ligne', () => {
  it('distingue une coupure passagère d’un vrai refus du serveur', () => {
    expect([0, 401, 502, 503, 504].map((s) => retryable(new ApiError(s, 'x', 'x')))).toEqual([true, true, true, true, true])
    expect([400, 404, 500].map((s) => retryable(new ApiError(s, 'x', 'x')))).toEqual([false, false, false])
    expect(retryable(new TypeError('Failed to fetch'))).toBe(true)
  })

  it('garde la dernière page par tome, l’envoie, s’arrête à la première coupure', async () => {
    queueProgress('a', 'p1')
    queueProgress('a', 'p2')
    queueProgress('b', 'p9')
    expect(pendingPage('a')).toBe('p2')
    // Réseau toujours absent : rien n'est perdu.
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))))
    await flushProgress()
    expect([pendingPage('a'), pendingPage('b')]).toEqual(['p2', 'p9'])
    // De retour : `a` accepté, `b` refusé (tome disparu) et abandonné.
    const fetch = vi.fn(async (url: string) => (url.includes('/a/') ? json({}) : json({ code: 'not_found', message: 'x' }, 404)))
    vi.stubGlobal('fetch', fetch)
    await flushProgress()
    // Avec l'heure de lecture : le serveur ne remplace pas une page lue depuis ailleurs.
    const sent = fetch.mock.calls.find(([url]) => url === '/api/volumes/a/progress') as unknown as [string, RequestInit]
    expect(sent[1].method).toBe('PUT')
    expect(JSON.parse(sent[1].body as string)).toEqual({ page_id: 'p2', read_at: expect.stringMatching(/^\d{4}-\d\d-\d\dT[\d:.]+Z$/) })
    expect([pendingPage('a'), pendingPage('b')]).toEqual([undefined, undefined])
  })

  it('une file par compte, envoyée seulement avec le compte qui l’a remplie', async () => {
    const fetch = vi.fn(async () => json({}))
    vi.stubGlobal('fetch', fetch)
    queueProgress('a', 'p1')
    setProgressUser(2)
    expect(pendingPage('a')).toBeUndefined()
    queueProgress('b', 'p2')
    await flushProgress()
    await vi.waitFor(() => expect(pendingPage('b')).toBeUndefined())
    expect(fetch).toHaveBeenCalledTimes(1)
    // Sans compte connu : gardée sans propriétaire, elle revient au prochain compte connecté.
    setProgressUser(null)
    queueProgress('c', 'p3')
    expect(pendingPage('c')).toBe('p3')
    // Le compte 1 revient : sa page et celle sans propriétaire partent.
    setProgressUser(1)
    await vi.waitFor(() => expect(pendingPage('a')).toBeUndefined())
    expect(fetch).toHaveBeenCalledWith('/api/volumes/a/progress', expect.anything())
    expect(fetch).toHaveBeenCalledWith('/api/volumes/c/progress', expect.anything())
  })

  it('la file d’avant les comptes revient au premier compte connecté', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))))
    localStorage.setItem('shiori.pendingProgress', JSON.stringify({ a: 'p1' }))
    setProgressUser(3)
    expect(pendingPage('a')).toBe('p1')
    expect(localStorage.getItem('shiori.pendingProgress')).toBeNull()
    // Page gardée par une version sans l'heure : envoyée sans heure.
    const fetch = vi.fn(async () => json({}))
    vi.stubGlobal('fetch', fetch)
    await flushProgress()
    expect(fetch).toHaveBeenCalledWith('/api/volumes/a/progress', expect.objectContaining({ body: JSON.stringify({ page_id: 'p1' }) }))
    // Illisible : abandonnée sans rien casser.
    localStorage.setItem('shiori.pendingProgress', '{pas du json')
    setProgressUser(3)
    expect(localStorage.getItem('shiori.pendingProgress')).toBeNull()
  })

  it('un stockage illisible ou refusé ne casse rien', async () => {
    localStorage.setItem('shiori.pendingProgress.1', '{pas du json')
    expect(pendingPage('a')).toBeUndefined()
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('plein')
    })
    queueProgress('a', 'p1')
    expect(pendingPage('a')).toBeUndefined()
  })
})
