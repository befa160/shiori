import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, SeriesSummary } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const libraries: Library[] = [
  { id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 2, volume_count: 2 },
  { id: 2, name: 'Comics', root: '/c', kind: 'comics', series_count: 1, volume_count: 1 },
]
const base = {
  library_id: 1,
  volume_count: 1,
  read_count: 0,
  page_count: 30,
  pages_read: 0,
  reading_status: 'unread',
  publication_status: null,
  created_at: '2026-01-01T00:00:00.000Z',
  last_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
} as const
const series: SeriesSummary[] = [
  { ...base, id: 1, name: 'Akira', reading_status: 'read', read_count: 1, publication_status: 'completed' },
  { ...base, id: 2, name: 'Été en pente', created_at: '2026-11-01T00:00:00.000Z' },
  { ...base, id: 3, name: 'Watchmen', library_id: 2, publication_status: 'completed' },
]

const serve = (libs = libraries) => mockApi({ '/api/libraries': () => json(libs), '/api/series': () => json(series) })
const grid = () => within(screen.getByRole('main')).getAllByRole('listitem').map((li) => within(li).getAllByTitle(/./)[0].textContent)
const search = () => screen.getByRole('searchbox', { name: 'Rechercher une série' })

describe('séries : recherche, filtres et tri', () => {
  it('cherche à la frappe, sans accents, et garde la recherche dans l’adresse', async () => {
    serve()
    const { router } = renderAt('/series')
    expect(await screen.findByRole('heading', { level: 1, name: 'Séries' })).toBeInTheDocument()
    expect(await screen.findByText('3 séries')).toHaveAttribute('aria-live', 'polite')
    await userEvent.type(search(), 'ete')
    expect(grid()).toEqual(['Été en pente'])
    expect(screen.getByText('1 série')).toBeInTheDocument()
    expect(router.state.location.search).toBe('?q=ete')
    // Taper ne remplit pas l'historique.
    expect(router.state.historyAction).toBe('REPLACE')
  })

  it('filtre et trie depuis les listes, et efface les filtres en gardant le tri', async () => {
    serve()
    const { router } = renderAt('/series')
    await screen.findByText('3 séries')
    await userEvent.click(screen.getByRole('combobox', { name: 'Publication' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Publication terminée' }))
    expect(grid()).toEqual(['Akira', 'Watchmen'])
    expect(screen.getByRole('combobox', { name: 'Publication' })).toHaveClass('text-primary')
    await userEvent.click(screen.getByRole('combobox', { name: 'Bibliothèque' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Comics' }))
    expect(grid()).toEqual(['Watchmen'])
    await userEvent.click(screen.getByRole('combobox', { name: 'Tri' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Récemment ajoutées' }))
    expect(new URLSearchParams(router.state.location.search).get('sort')).toBe('added')

    await userEvent.click(screen.getByRole('button', { name: 'Effacer les filtres' }))
    expect(router.state.location.search).toBe('?sort=added')
    expect(grid()).toEqual(['Été en pente', 'Akira', 'Watchmen'])
    expect(screen.queryByRole('button', { name: 'Effacer les filtres' })).toBeNull()
  })

  it('applique les filtres d’un lien, et propose de les effacer quand rien ne correspond', async () => {
    serve()
    const { router } = renderAt('/series?read=read&q=watch')
    expect(await screen.findByText('Aucune série ne correspond.')).toBeInTheDocument()
    expect(screen.getByText('0 série')).toBeInTheDocument()
    expect(search()).toHaveValue('watch')
    expect(screen.getByRole('combobox', { name: 'Lecture' })).toHaveTextContent('Lues')
    await userEvent.click(within(screen.getByRole('main')).getAllByRole('button', { name: 'Effacer les filtres' })[1])
    expect(router.state.location.search).toBe('')
    expect(grid()).toHaveLength(3)
    expect(search()).toHaveValue('')
  })

  it('ne propose pas de choisir la bibliothèque quand il n’y en a qu’une', async () => {
    serve([libraries[0]])
    renderAt('/series')
    await screen.findByText('3 séries')
    expect(screen.queryByRole('combobox', { name: 'Bibliothèque' })).toBeNull()
  })

  it('l’onglet Recherche ouvre le même écran, champ prêt à taper', async () => {
    serve()
    const { router } = renderAt('/')
    const tabs = screen.getAllByRole('navigation', { name: 'Navigation principale' })[1]
    await userEvent.click(within(tabs).getByRole('link', { name: 'Recherche' }))
    expect(router.state.location.pathname).toBe('/search')
    expect(await screen.findByRole('heading', { level: 1, name: 'Recherche' })).toBeInTheDocument()
    expect(search()).toHaveFocus()
    expect(within(tabs).getByRole('link', { name: 'Recherche' })).toHaveAttribute('aria-current', 'page')
    expect(within(tabs).getByRole('link', { name: 'Séries' })).not.toHaveAttribute('aria-current')
  })
})
