import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi } from '@/test/render'
import { ApiError, WIDTHS, api, get, pageImageUrl } from './client'

afterEach(() => vi.unstubAllGlobals())

describe('get', () => {
  it('renvoie le JSON et demande du JSON', async () => {
    const fetch = mockApi({ '/api/libraries': () => json([{ id: 1, name: 'Manga' }]) })
    expect(await api.libraries()).toEqual([{ id: 1, name: 'Manga' }])
    expect(fetch.mock.calls[0][1]).toMatchObject({ headers: { accept: 'application/json' } })
  })

  it('renvoie null pour un 204', async () => {
    mockApi({ '/api/vide': () => new Response(null, { status: 204 }) })
    expect(await get('/vide')).toBeNull()
  })

  it("garde le code et le message d'une erreur de l'API", async () => {
    mockApi({ '/api/volumes/v1': () => json({ code: 'volume_changed', message: 'à rescanner' }, 409) })
    const error = await api.volume('v1').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 409, code: 'volume_changed', message: 'à rescanner' })
  })

  it("donne un code générique à une erreur qui n'est pas du JSON de l'API", async () => {
    mockApi({ '/api/health': () => new Response('<html>proxy</html>', { status: 502 }) })
    await expect(api.health()).rejects.toMatchObject({ status: 502, code: 'http_error', message: 'erreur HTTP 502' })
  })

  it('signale un serveur injoignable', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    await expect(api.health()).rejects.toMatchObject({ status: 0, code: 'network' })
  })

  it('construit les chemins de chaque route', async () => {
    const fetch = mockApi({})
    for (const call of [
      () => api.librarySeries(3),
      () => api.series(4),
      () => api.pages('abc'),
    ]) {
      await call().catch(() => undefined)
    }
    expect(fetch.mock.calls.map((c) => c[0])).toEqual([
      '/api/libraries/3/series',
      '/api/series/4',
      '/api/volumes/abc/pages',
    ])
  })
})

describe('pageImageUrl', () => {
  it("donne l'originale ou une largeur de la liste fixe", () => {
    expect(pageImageUrl('p1')).toBe('/api/pages/p1/image')
    expect(pageImageUrl('p1', { width: 640, format: 'avif' })).toBe('/api/pages/p1/image?w=640&f=avif')
    expect(WIDTHS).toContain(2048)
  })
})
