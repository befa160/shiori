import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/client'
import { json } from '@/test/render'
import { flushProgress, pendingPage, queueProgress, retryable } from './progress'

beforeEach(() => localStorage.clear())
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
    expect(fetch).toHaveBeenCalledWith('/api/volumes/a/progress', expect.objectContaining({ method: 'PUT', body: JSON.stringify({ page_id: 'p2' }) }))
    expect([pendingPage('a'), pendingPage('b')]).toEqual([undefined, undefined])
  })

  it('un stockage illisible ou refusé ne casse rien', async () => {
    localStorage.setItem('shiori.pendingProgress', '{pas du json')
    expect(pendingPage('a')).toBeUndefined()
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('plein')
    })
    queueProgress('a', 'p1')
    expect(pendingPage('a')).toBeUndefined()
  })
})
