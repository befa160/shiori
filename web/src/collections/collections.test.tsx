import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Collection, SeriesDetail, SeriesSummary } from '@/api/types'
import { json, mockApi, NO_METADATA, READER, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const summary = {
  library_id: 1,
  volume_count: 1,
  read_count: 0,
  page_count: 30,
  pages_read: 0,
  reading_status: 'unread',
  publication_status: null,
  created_at: '2026-11-01T00:00:00.000Z',
  last_read_at: null,
  to_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
  cover_image_id: null,
  alt_titles: [] as string[],
  authors: [] as string[],
  tags: [] as string[],
} as const
const series: SeriesSummary[] = [
  { ...summary, id: 7, name: 'Toriko', cover_page_id: 'c7' },
  { ...summary, id: 8, name: 'Bleach', cover_page_id: 'c8' },
  { ...summary, id: 9, name: 'Hors collection' },
]
const shonen: Collection = { id: 2, name: 'Shōnen', series_ids: [7, 8] }
const empty: Collection = { id: 3, name: 'Vide', series_ids: [] }
const main = () => within(screen.getByRole('main'))
const asReader = { auth: { setup_required: false, user: READER } }

describe('collections', () => {
  it('liste les collections, couverture de la première série par titre', async () => {
    mockApi({ '/api/collections': () => json([shonen, empty]), '/api/series': () => json(series) })
    renderAt('/collections')
    const card = await main().findByRole('link', { name: /Shōnen/ })
    expect(card).toHaveAttribute('href', '/collections/2')
    expect(card).toHaveTextContent('2 séries')
    await waitFor(() => expect(card.querySelector('img')).toHaveAttribute('src', '/api/pages/c8/image?w=320&f=jpeg'))
    expect(main().getByRole('link', { name: /Vide/ })).toHaveTextContent('0 série')
  })

  it('crée une collection et y mène ; un nom pris se dit', async () => {
    let taken = true
    const fetch = mockApi({
      '/api/collections': (init) =>
        init?.method === 'POST'
          ? taken
            ? json({ code: 'name_taken', message: 'x' }, 409)
            : json({ id: 4, name: 'Seinen', series_ids: [] }, 201)
          : json([]),
      '/api/series': () => json(series),
    })
    const { router } = renderAt('/collections')
    expect(await main().findByText(/Aucune collection pour l’instant. Créez-en une ici/)).toBeInTheDocument()
    await userEvent.click(main().getByRole('button', { name: 'Nouvelle collection' }))
    const dialog = await screen.findByRole('dialog', { name: 'Nouvelle collection' })
    await userEvent.type(within(dialog).getByLabelText('Nom'), 'Seinen')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Une collection porte déjà ce nom')
    taken = false
    await userEvent.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(router.state.location.pathname).toBe('/collections/4'))
    const sent = fetch.mock.calls.filter(([, init]) => init?.method === 'POST').map(([, init]) => JSON.parse(init?.body as string))
    expect(sent).toEqual([{ name: 'Seinen' }, { name: 'Seinen' }])
  })

  it('montre les séries d’une collection par titre ; l’administrateur la renomme et la supprime', async () => {
    let list = [shonen]
    const fetch = mockApi({
      '/api/collections': () => json(list),
      '/api/series': () => json(series),
      '/api/collections/2': (init) => {
        if (init?.method === 'PATCH') {
          list = [{ ...shonen, name: 'Nekketsu' }]
          return json(list[0])
        }
        list = []
        return new Response(null, { status: 204 })
      },
    })
    const { router } = renderAt('/collections/2')
    await main().findByRole('heading', { level: 1, name: 'Shōnen' })
    const names = main().getAllByRole('link', { name: /Toriko|Bleach/ }).map((l) => l.getAttribute('href'))
    expect(names).toEqual(['/series/8', '/series/7'])
    expect(main().queryByRole('link', { name: /Hors collection/ })).toBeNull()

    await userEvent.click(main().getByRole('button', { name: 'Renommer' }))
    const dialog = await screen.findByRole('dialog', { name: 'Renommer « Shōnen »' })
    const input = within(dialog).getByLabelText('Nom')
    await userEvent.clear(input)
    await userEvent.type(input, 'Nekketsu')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    expect(await main().findByRole('heading', { level: 1, name: 'Nekketsu' })).toBeInTheDocument()

    await userEvent.click(main().getByRole('button', { name: 'Supprimer' }))
    const confirm = await screen.findByRole('alertdialog', { name: 'Supprimer la collection « Nekketsu » ?' })
    await userEvent.click(within(confirm).getByRole('button', { name: 'Supprimer' }))
    await waitFor(() => expect(router.state.location.pathname).toBe('/collections'))
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/collections/2' && init?.method === 'DELETE')).toBe(true)
  })

  it('un lecteur voit les séries sans les boutons ; une collection inconnue se dit', async () => {
    mockApi({ '/api/collections': () => json([{ ...shonen, series_ids: [] }]), '/api/series': () => json(series) })
    const { router } = renderAt('/collections/2', asReader)
    expect(await main().findByText('Aucune série dans cette collection.')).toBeInTheDocument()
    expect(main().queryByRole('button', { name: 'Renommer' })).toBeNull()
    await router.navigate('/collections/99')
    expect(await main().findByRole('alert')).toHaveTextContent('Collection introuvable')
  })

  it('étagère « Collections » à l’accueil, sans les collections vides', async () => {
    mockApi({
      '/api/libraries': () => json([]),
      '/api/series': () => json(series),
      '/api/continue': () => json([]),
      '/api/collections': () => json([shonen, empty]),
    })
    renderAt('/')
    const shelf = within(await screen.findByRole('region', { name: 'Collections' }))
    expect(shelf.getByRole('link', { name: /Shōnen/ })).toHaveAttribute('href', '/collections/2')
    expect(shelf.queryByRole('link', { name: /Vide/ })).toBeNull()
    expect(shelf.getByRole('link', { name: 'Tout voir : Collections' })).toHaveAttribute('href', '/collections')
  })
})

const detail: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  publication_status: null,
  metadata: NO_METADATA,
  locked: [],
  sources: [],
  followed: false,
  to_read: false,
  collections: [{ id: 2, name: 'Shōnen' }],
  resume_volume_id: null,
  page_count: 30,
  pages_read: 0,
  settings: { reading_mode: 'rtl', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null },
  volumes: [{ volume_id: 'a', kind: 'volume', number: 1, number_end: null, status: 'ok', page_count: 30, cover_page_id: null, file_name: 'T01.cbz' }],
}

describe('collections d’une série', () => {
  it('montre ses collections ; l’administrateur coche, décoche et crée avec la série', async () => {
    let list: Collection[] = [{ ...shonen, series_ids: [7] }, { id: 3, name: 'Vide', series_ids: [] }]
    const fetch = mockApi({
      '/api/series/7': () => json(detail),
      '/api/collections': (init) => {
        if (init?.method === 'POST') return json({ id: 5, name: 'Gourmet', series_ids: [] }, 201)
        return json(list)
      },
      '/api/collections/3/series/7': () => {
        list = [list[0], { ...list[1], series_ids: [7] }]
        return json(list[1])
      },
      '/api/collections/5/series/7': () => json({ id: 5, name: 'Gourmet', series_ids: [7] }),
    })
    renderAt('/series/7')
    const chips = within(await main().findByRole('list', { name: 'Collections' }))
    expect(chips.getByRole('link', { name: 'Shōnen' })).toHaveAttribute('href', '/collections/2')

    await userEvent.click(main().getByRole('button', { name: 'Actions pour Toriko' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Collections…' }))
    const dialog = await screen.findByRole('dialog', { name: 'Collections de « Toriko »' })
    expect(within(dialog).getByRole('checkbox', { name: 'Shōnen' })).toBeChecked()
    const vide = within(dialog).getByRole('checkbox', { name: 'Vide' })
    await userEvent.click(vide)
    await waitFor(() => expect(vide).toBeChecked())
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/collections/3/series/7' && init?.method === 'PUT')).toBe(true)

    await userEvent.type(within(dialog).getByLabelText('Nouvelle collection, avec cette série'), 'Gourmet')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Créer' }))
    await waitFor(() => expect(fetch.mock.calls.some(([url, init]) => url === '/api/collections/5/series/7' && init?.method === 'PUT')).toBe(true))
  })

  it('un lecteur n’a pas l’entrée « Collections… »', async () => {
    mockApi({ '/api/series/7': () => json(detail) })
    renderAt('/series/7', asReader)
    await main().findByRole('list', { name: 'Collections' })
    await userEvent.click(main().getByRole('button', { name: 'Actions pour Toriko' }))
    await screen.findByRole('menuitem', { name: 'Marquer comme lu' })
    expect(screen.queryByRole('menuitem', { name: 'Collections…' })).toBeNull()
  })
})

