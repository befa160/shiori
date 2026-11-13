import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, SeriesDetail, SeriesSummary } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const library: Library = { id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 2, volume_count: 1 }
const longName =
  'Une série au titre extrêmement long, choisi pour vérifier que la troncature sur deux lignes tient sur tous les écrans'
const summary = {
  library_id: 1,
  read_count: 0,
  reading_status: 'unread',
  publication_status: null,
  created_at: '2026-11-12T00:00:00.000Z',
  last_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
} as const
const series: SeriesSummary[] = [
  { ...summary, id: 7, name: 'Toriko', volume_count: 1 },
  { ...summary, id: 8, name: longName, volume_count: 3 },
]
const settings = { reading_mode: 'rtl', page_layout: 'auto', reading_mode_override: null, page_layout_override: null } as const
const detail: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  publication_status: null,
  settings,
  volumes: [
    { volume_id: 'a', kind: 'volume', number: 1, number_end: null, status: 'ok', page_count: 200, cover_page_id: null, file_name: 'Toriko T01.cbz', progress: { page_id: 'p', ordinal: 199, page_count: 200, finished: true, updated_at: '' } },
    { volume_id: 'b', kind: 'volume', number: 2, number_end: null, status: 'warning', page_count: 180, cover_page_id: null, file_name: 'Toriko T02.cbz', progress: { page_id: 'q', ordinal: 44, page_count: 180, finished: false, updated_at: '' } },
    { volume_id: 'c', kind: 'unknown', number: null, number_end: null, status: 'error', page_count: 12, cover_page_id: null, file_name: 'bonus.cbr' },
  ],
}

describe('séries d’une bibliothèque', () => {
  it('montre les séries en grille de couvertures', async () => {
    const read = { ...series[0], reading_status: 'read', read_count: 1, cover_page_id: 'c7', resume_volume_id: 'v7' } as const
    const reading = { ...series[1], reading_status: 'in_progress', read_count: 1 } as const
    mockApi({ '/api/libraries': () => json([library]), '/api/libraries/1/series': () => json([read, reading]) })
    renderAt('/libraries/1')
    const main = within(screen.getByRole('main'))
    const toriko = await main.findByRole('link', { name: /^Toriko/ })
    expect(toriko).toHaveAttribute('href', '/series/7')
    expect(toriko).toHaveTextContent('Lu')
    // Image décorative (`alt=""`) : pas de rôle `img`, le lien porte déjà le titre.
    expect(toriko.querySelector('img')).toHaveAttribute('src', '/api/pages/c7/image?w=320&f=jpeg')
    expect(main.getByRole('link', { name: 'Lire Toriko' })).toHaveAttribute('href', '/read/v7')
    const long = main.getByRole('link', { name: new RegExp(longName.slice(0, 20)) })
    expect(within(long).getByTitle(longName)).toHaveClass('line-clamp-2')
    expect(within(long).getByRole('progressbar', { name: '1 tome lu sur 3' })).toHaveAttribute('aria-valuenow', '33')
    // Sans couverture : couverture générée ; sans tome à reprendre : pas de bouton « Lire ».
    expect(within(long).getByTestId('fallback-cover')).toHaveTextContent(longName)
    expect(main.queryByRole('link', { name: `Lire ${longName}` })).toBeNull()
  })

  it("revient à l'accueil et signale une bibliothèque vide", async () => {
    mockApi({ '/api/libraries': () => json([library]), '/api/libraries/1/series': () => json([]) })
    const { router } = renderAt('/libraries/1')
    expect(await screen.findByText(/Aucune série/)).toBeInTheDocument()
    await userEvent.click(within(screen.getByRole('main')).getByRole('link', { name: 'Accueil' }))
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
    const items = await within(screen.getByRole('main')).findAllByRole('listitem')
    expect(items.map((li) => within(li).getAllByTitle(/./)[0].textContent)).toEqual(['Tome 1', 'Tome 2', 'bonus.cbr'])
    expect(within(items[0]).getByText('Lu')).toBeInTheDocument()
    expect(within(items[0]).queryByRole('progressbar')).toBeNull()
    expect(within(items[1]).getByText('Avertissement')).toBeInTheDocument()
    expect(within(items[1]).getByText('45 / 180')).toBeInTheDocument()
    expect(within(items[1]).getByRole('progressbar', { name: 'Progression de Tome 2' })).toBeInTheDocument()
    expect(within(items[1]).getByText(/Toriko T02.cbz · 180 pages/)).toBeInTheDocument()
    expect(within(items[2]).getByText('Erreur')).toBeInTheDocument()
    expect(within(items[2]).getByText('12 pages')).toBeInTheDocument()
    expect(within(items[0]).getByRole('link')).toHaveAttribute('href', '/read/a')
    // Sans la liste des bibliothèques (non fournie ici), le retour s'appelle « Bibliothèque ».
    await userEvent.click(within(screen.getByRole('main')).getByRole('link', { name: 'Bibliothèque' }))
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
    expect(within(screen.getByRole('main')).getByRole('link', { name: 'Accueil' })).toHaveAttribute('href', '/')
  })
})

describe('identifiant invalide dans l’adresse', () => {
  it.each(['/series/abc', '/libraries/0', '/series/1.5'])('%s : page introuvable, sans appeler l’API', async (path) => {
    const fetch = mockApi({})
    renderAt(path)
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    // Seuls les réglages (langue) et les bibliothèques (sidebar) sont demandés, jamais la ressource.
    expect(new Set(fetch.mock.calls.map((c) => c[0]))).toEqual(new Set(['/api/settings', '/api/libraries']))
  })
})
