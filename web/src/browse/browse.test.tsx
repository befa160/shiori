import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, SeriesDetail, SeriesSummary } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const library: Library = { id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 2, volume_count: 1 }
const longName =
  'Une série au titre extrêmement long, choisi pour vérifier que la troncature sur deux lignes tient sur tous les écrans'
const series: SeriesSummary[] = [
  { id: 7, name: 'Toriko', volume_count: 1, cover_page_id: null },
  { id: 8, name: longName, volume_count: 3, cover_page_id: null },
]
const settings = { reading_mode: 'rtl', page_layout: 'auto', reading_mode_override: null, page_layout_override: null } as const
const detail: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  settings,
  volumes: [
    { volume_id: 'a', kind: 'volume', number: 1, number_end: null, status: 'ok', page_count: 200, cover_page_id: null, file_name: 'Toriko T01.cbz', progress: { page_id: 'p', ordinal: 199, page_count: 200, finished: true, updated_at: '' } },
    { volume_id: 'b', kind: 'volume', number: 2, number_end: null, status: 'warning', page_count: 180, cover_page_id: null, file_name: 'Toriko T02.cbz', progress: { page_id: 'q', ordinal: 44, page_count: 180, finished: false, updated_at: '' } },
    { volume_id: 'c', kind: 'unknown', number: null, number_end: null, status: 'error', page_count: 12, cover_page_id: null, file_name: 'bonus.cbr' },
  ],
}

describe('bibliothèques', () => {
  it('liste chaque bibliothèque avec son type et ses compteurs, puis ouvre ses séries', async () => {
    mockApi({ '/api/libraries': () => json([library]), '/api/libraries/1/series': () => json(series) })
    const { router } = renderAt('/')
    expect(screen.getByText('Chargement…').parentElement).toHaveAttribute('aria-busy', 'true')
    const row = await screen.findByRole('link', { name: /Mangas/ })
    expect(row).toHaveTextContent('Manga · 2 séries · 1 tome')
    await userEvent.click(row)
    expect(router.state.location.pathname).toBe('/libraries/1')
    expect(await screen.findByRole('heading', { name: 'Mangas' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /Toriko/ })).toHaveTextContent('1 tome')
    const long = screen.getByRole('link', { name: new RegExp(longName.slice(0, 20)) })
    expect(within(long).getByTitle(longName)).toHaveClass('line-clamp-2')
  })

  it('explique quoi faire quand il n’y en a aucune', async () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/')
    expect(await screen.findByText("Aucune bibliothèque pour l'instant.")).toBeInTheDocument()
  })

  it('affiche une erreur et réessaie, en le montrant pendant le nouvel essai', async () => {
    let calls = 0
    let release = () => {}
    mockApi({
      '/api/libraries': () =>
        ++calls === 1
          ? json({ code: 'internal', message: 'erreur interne' }, 500)
          : new Promise<Response>((resolve) => (release = () => resolve(json([library])))),
    })
    renderAt('/')
    expect(await screen.findByRole('alert')).toHaveTextContent('Erreur interne du serveur')
    await userEvent.click(screen.getByRole('button', { name: 'Réessayer' }))
    expect(await screen.findByText('Chargement…')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Réessayer' })).toBeNull()
    release()
    expect(await screen.findByRole('link', { name: /Mangas/ })).toBeInTheDocument()
  })
})

describe('séries d’une bibliothèque', () => {
  it('revient aux bibliothèques et signale une bibliothèque vide', async () => {
    mockApi({ '/api/libraries': () => json([library]), '/api/libraries/1/series': () => json([]) })
    const { router } = renderAt('/libraries/1')
    expect(await screen.findByText(/Aucune série/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: 'Bibliothèques' }))
    expect(router.state.location.pathname).toBe('/')
  })

  it("dit qu'une bibliothèque n'existe pas", async () => {
    mockApi({
      '/api/libraries': () => json([]),
      '/api/libraries/9/series': () => json({ code: 'not_found', message: 'bibliothèque introuvable' }, 404),
    })
    renderAt('/libraries/9')
    expect(await screen.findByRole('alert')).toHaveTextContent('Bibliothèque introuvable')
    expect(screen.getByRole('heading', { name: 'Bibliothèque' })).toBeInTheDocument()
  })
})

describe('tomes d’une série', () => {
  it('montre chaque tome dans l’ordre du serveur, son état et sa progression', async () => {
    mockApi({ '/api/series/7': () => json(detail) })
    const { router } = renderAt('/series/7')
    const items = await screen.findAllByRole('listitem')
    expect(items.map((li) => within(li).getAllByTitle(/./)[0].textContent)).toEqual(['Tome 1', 'Tome 2', 'bonus.cbr'])
    expect(within(items[0]).getByText('Lu')).toBeInTheDocument()
    expect(within(items[0]).queryByRole('progressbar')).toBeNull()
    expect(within(items[1]).getByText('Avertissement')).toBeInTheDocument()
    expect(within(items[1]).getByText('45 / 180')).toBeInTheDocument()
    expect(within(items[1]).getByRole('progressbar', { name: 'Progression de Tome 2' })).toBeInTheDocument()
    expect(within(items[1]).getByText(/Toriko T02.cbz · 180 pages/)).toBeInTheDocument()
    expect(within(items[2]).getByText('Erreur')).toBeInTheDocument()
    expect(within(items[2]).getByText('12 pages')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: 'Séries' }))
    expect(router.state.location.pathname).toBe('/libraries/1')
  })

  it('signale une série sans tome', async () => {
    mockApi({ '/api/series/7': () => json({ ...detail, volumes: [] }) })
    renderAt('/series/7')
    expect(await screen.findByText('Aucun tome dans cette série.')).toBeInTheDocument()
  })

  it('revient à l’accueil si la série est inconnue', async () => {
    mockApi({ '/api/series/9': () => json({ code: 'not_found', message: 'série introuvable' }, 404) })
    renderAt('/series/9')
    expect(await screen.findByRole('alert')).toHaveTextContent('Série introuvable')
    expect(screen.getByRole('link', { name: 'Séries' })).toHaveAttribute('href', '/')
  })
})

describe('identifiant invalide dans l’adresse', () => {
  it.each(['/series/abc', '/libraries/0', '/series/1.5'])('%s : page introuvable, sans appeler l’API', async (path) => {
    const fetch = mockApi({})
    renderAt(path)
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    // Seuls les réglages (langue) sont demandés, jamais la ressource.
    expect(fetch.mock.calls.map((c) => c[0])).toEqual(['/api/settings'])
  })
})
