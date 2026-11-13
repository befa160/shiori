import { fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ContinueItem, Library, SeriesSummary, SeriesVolume } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'
import { titleHue } from './titleHue'

afterEach(() => vi.unstubAllGlobals())

const libraries: Library[] = [
  { id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 2, volume_count: 3 },
  { id: 2, name: 'Vide', root: '/v', kind: 'comics', series_count: 0, volume_count: 0 },
]
const summary = {
  library_id: 1,
  volume_count: 2,
  read_count: 0,
  page_count: 30,
  pages_read: 0,
  reading_status: 'unread',
  publication_status: null,
  last_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
  alt_titles: [] as string[],
  authors: [] as string[],
} as const
const series: SeriesSummary[] = [
  { ...summary, id: 7, name: 'Ancienne', created_at: '2026-01-01T00:00:00.000Z' },
  { ...summary, id: 8, name: 'Récente', created_at: '2026-11-01T00:00:00.000Z', cover_page_id: 'c8' },
]
const volume: SeriesVolume = {
  volume_id: 'v2',
  kind: 'volume',
  number: 2,
  number_end: null,
  status: 'ok',
  page_count: 180,
  cover_page_id: null,
  file_name: 'T02.cbz',
}
const reading: ContinueItem[] = [
  {
    series_id: 7,
    series_name: 'Ancienne',
    library_id: 1,
    updated_at: '2026-11-12T10:00:00.000Z',
    volume: { ...volume, progress: { page_id: 'p', ordinal: 44, page_count: 180, finished: false, updated_at: '' } },
  },
  { series_id: 8, series_name: 'Récente', library_id: 1, updated_at: '2026-11-11T10:00:00.000Z', volume: { ...volume, volume_id: 'v3', number: 3 } },
]

const api = (continueItems: ContinueItem[] = reading) => ({
  '/api/libraries': () => json(libraries),
  '/api/series': () => json(series),
  '/api/continue': () => json(continueItems),
})

describe('accueil', () => {
  it('propose de continuer la lecture : tome en cours à sa page, tome suivant', async () => {
    mockApi(api())
    renderAt('/')
    const section = within(await screen.findByRole('region', { name: 'Continuer la lecture' }))
    const current = await section.findByRole('link', { name: /Ancienne/ })
    expect(current).toHaveAttribute('href', '/read/v2')
    expect(current).toHaveTextContent('Tome 2 · 45 / 180')
    expect(within(current).getByRole('progressbar', { name: 'Progression de Tome 2' })).toHaveAttribute('aria-valuenow', '25')
    const next = section.getByRole('link', { name: /Récente/ })
    expect(next).toHaveAttribute('href', '/read/v3')
    expect(next).toHaveTextContent('Suivant')
    expect(within(next).queryByRole('progressbar')).toBeNull()
  })

  it('montre les derniers ajouts de chaque bibliothèque et mène à la grille', async () => {
    mockApi(api([]))
    const { router } = renderAt('/')
    const shelf = within(await screen.findByRole('region', { name: 'Mangas' }))
    const titles = shelf.getAllByRole('listitem').map((li) => within(li).getAllByTitle(/./)[0].textContent)
    expect(titles).toEqual(['Récente', 'Ancienne'])
    expect(screen.queryByRole('region', { name: 'Continuer la lecture' })).toBeNull()
    expect(within(screen.getByRole('region', { name: 'Vide' })).getByText(/Aucune série/)).toBeInTheDocument()
    await userEvent.click(shelf.getByRole('link', { name: 'Tout voir : Mangas' }))
    expect(router.state.location.pathname).toBe('/libraries/1')
  })

  it('remplace une couverture qui ne se charge pas par la couverture générée', async () => {
    mockApi(api([]))
    renderAt('/')
    const card = await within(await screen.findByRole('region', { name: 'Mangas' })).findByRole('link', { name: /Récente/ })
    const img = card.querySelector('img') as HTMLImageElement
    expect(img).toHaveAttribute('srcset', expect.stringContaining('/api/pages/c8/image?w=480&f=jpeg 480w'))
    expect(img).toHaveAttribute('loading', 'lazy')
    fireEvent.error(img)
    expect(within(card).getByTestId('fallback-cover')).toHaveTextContent('Récente')
  })

  it('explique quoi faire sans bibliothèque', async () => {
    mockApi({ '/api/libraries': () => json([]), '/api/series': () => json([]), '/api/continue': () => json([]) })
    renderAt('/')
    expect(await screen.findByText("Aucune bibliothèque pour l'instant.")).toBeInTheDocument()
  })

  it('affiche une erreur et réessaie, avec des couvertures vides pendant le nouvel essai', async () => {
    let calls = 0
    let release = () => {}
    mockApi({
      ...api([]),
      '/api/libraries': () =>
        ++calls === 1
          ? json({ code: 'internal', message: 'erreur interne' }, 500)
          : new Promise<Response>((resolve) => (release = () => resolve(json(libraries)))),
    })
    renderAt('/')
    const main = within(screen.getByRole('main'))
    expect(await main.findByRole('alert')).toHaveTextContent('Erreur interne du serveur')
    await userEvent.click(main.getByRole('button', { name: 'Réessayer' }))
    expect(await main.findByText('Chargement…')).toBeInTheDocument()
    release()
    expect(await screen.findByRole('region', { name: 'Mangas' })).toBeInTheDocument()
  })
})

describe('couverture générée', () => {
  it('garde la même teinte pour un titre, et en change pour un autre', () => {
    expect(titleHue('One Piece')).toBe(titleHue('One Piece'))
    expect(titleHue('One Piece')).not.toBe(titleHue('Toriko'))
    expect(titleHue('')).toBe(0)
  })
})
