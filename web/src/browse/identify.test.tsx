import { fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SeriesDetail, SourceState, WebMetadata } from '@/api/types'
import { NO_METADATA, json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const settings = { reading_mode: 'rtl', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null } as const
const base: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  publication_status: null,
  metadata: NO_METADATA,
  locked: [],
  sources: [],
  resume_volume_id: null,
  page_count: 0,
  pages_read: 0,
  settings,
  volumes: [],
}
const fiche = (id: string, title: string, year: number | null = 2008): WebMetadata => ({
  id,
  titles: [{ lang: 'ja-ro', text: title }],
  summaries: [],
  authors: [{ name: 'Mitsutoshi Shimabukuro', story: true, art: true }],
  genres: [],
  tags: [],
  year,
  status: null,
  links: [],
})
const state = (over: Partial<SourceState>): SourceState => ({
  source: 'anilist',
  external_id: null,
  matched_by: null,
  checked_at: '',
  last_error: null,
  candidates: [],
  ...over,
})
const online = (on: boolean) => () => json({ language: 'fr', page_turn: 'reading', metadata_online: on, metadata_sources: null })
const main = () => within(screen.getByRole('main'))
const dialog = async () => within(await screen.findByRole('dialog', { name: 'Identifier « Toriko »' }))

/** Série servie par un faux serveur ; chaque action renvoie le détail que donne `after`. */
function serve(detail: SeriesDetail, answers: Record<string, (init?: RequestInit) => Response> = {}) {
  let current = detail
  const fetch = mockApi({
    '/api/settings': online(true),
    '/api/series/7': (init) => {
      if (init?.method === 'PATCH') current = { ...current, metadata: { ...current.metadata, ...JSON.parse(String(init.body)) } }
      return json(current)
    },
    ...answers,
  })
  const calls = (method: string) => fetch.mock.calls.filter(([, init]) => init?.method === method).map(([url, init]) => [url, init?.body ? JSON.parse(String(init.body)) : null])
  return { calls, set: (d: SeriesDetail) => (current = d) }
}

describe('identifier une série', () => {
  it('n’apparaît pas tant que le réseau est désactivé', async () => {
    mockApi({ '/api/settings': online(false), '/api/series/7': () => json(base) })
    renderAt('/series/7')
    expect(await main().findByRole('button', { name: 'Modifier' })).toBeInTheDocument()
    expect(main().queryByRole('button', { name: 'Identifier' })).not.toBeInTheDocument()
  })

  it('ouvre sur les fiches à confirmer, en vignettes relayées, et lie celle qu’on choisit', async () => {
    const pending = { ...base, sources: [state({ candidates: [fiche('1', 'Toriko'), fiche('2', 'Toriko Gaiden', null)] })] }
    const linked = { ...base, sources: [state({ external_id: '1', matched_by: 'user' })] }
    const { calls } = serve(pending, { '/api/series/7/metadata/links/anilist': () => json(linked) })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Identifier' }))
    const d = await dialog()
    expect(d.getByRole('combobox', { name: 'Source' })).toHaveTextContent('AniList')
    expect(d.getByRole('textbox', { name: 'Titre à chercher' })).toHaveValue('Toriko')
    expect(d.getByRole('heading', { name: 'Fiches possibles, à confirmer' })).toBeInTheDocument()
    const first = d.getByRole('button', { name: 'Choisir Toriko · 2008' })
    expect(first.querySelector('img')).toHaveAttribute('src', '/api/metadata/thumbnail/anilist/1')
    expect(d.getByRole('button', { name: 'Choisir Toriko Gaiden' })).toBeInTheDocument()
    // Crédits des sources.
    expect(d.getByRole('link', { name: 'MangaDex' })).toHaveAttribute('href', 'https://mangadex.org')
    await userEvent.click(first)
    expect(await screen.findByText('Série liée')).toBeInTheDocument()
    expect(calls('PUT')).toEqual([['/api/series/7/metadata/links/anilist', { external_id: '1' }]])
    expect(await d.findByText(/Liée à une fiche de AniList · choisie par vous/)).toBeInTheDocument()
    expect(d.getByRole('link', { name: 'Voir la fiche' })).toHaveAttribute('href', 'https://anilist.co/manga/1')
  })

  it('cherche dans la source choisie, dit quand rien ne correspond et quand la source échoue', async () => {
    let answer: () => Response = () => json([fiche('9', 'Toriko')])
    const url = '/api/series/7/metadata/search?source=mangadex&q=Toriko+Gourmet'
    serve(base, { [url]: () => answer() })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Identifier' }))
    const d = await dialog()
    // Français : MangaDex d'abord ; rien à confirmer, donc pas encore de liste.
    expect(d.getByRole('combobox', { name: 'Source' })).toHaveTextContent('MangaDex')
    expect(d.queryByRole('heading', { name: 'Résultats' })).not.toBeInTheDocument()
    const input = d.getByRole('textbox', { name: 'Titre à chercher' })
    await userEvent.clear(input)
    expect(d.getByRole('button', { name: 'Chercher' })).toBeDisabled()
    await userEvent.type(input, ' Toriko Gourmet {Enter}')
    expect(await d.findByRole('heading', { name: 'Résultats' })).toBeInTheDocument()
    const choice = d.getByRole('button', { name: 'Choisir Toriko · 2008' })
    // Vignette qui ne se charge pas : couverture de repli.
    fireEvent.error(choice.querySelector('img') as HTMLImageElement)
    expect(within(choice).getByTestId('fallback-cover')).toBeInTheDocument()

    answer = () => json([])
    await userEvent.click(d.getByRole('button', { name: 'Chercher' }))
    expect(await d.findByText(/Aucune fiche trouvée\. Essayez un autre titre/)).toBeInTheDocument()
    answer = () => json({ code: 'rate_limited', message: 'x' }, 503)
    await userEvent.click(d.getByRole('button', { name: 'Chercher' }))
    expect(await d.findByRole('alert')).toHaveTextContent('La source demande d’attendre')
    // Changer de source repart de ses propres fiches.
    await userEvent.click(d.getByRole('combobox', { name: 'Source' }))
    await userEvent.click(await screen.findByRole('option', { name: 'AniList' }))
    expect(d.queryByRole('heading', { name: 'Résultats' })).not.toBeInTheDocument()
  })

  it('actualise, prend la couverture ou délie une source liée, et dit un échec', async () => {
    const linked = { ...base, sources: [state({ source: 'mangadex', external_id: 'abc', matched_by: 'auto', last_error: 'offline' })] }
    const after = { ...linked, metadata: { ...NO_METADATA, cover_image_id: 'c1' } }
    let fail = false
    const { calls } = serve(linked, {
      '/api/series/7/metadata/refresh': () => (fail ? json({ code: 'offline', message: 'x' }, 502) : json(linked)),
      '/api/series/7/metadata/cover': () => json(after),
      '/api/series/7/metadata/links/mangadex': () => json(base),
    })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Identifier' }))
    const d = await dialog()
    expect(d.getByText(/Liée à une fiche de MangaDex · trouvée automatiquement/)).toBeInTheDocument()
    expect(d.getByText('Dernier essai : source injoignable')).toBeInTheDocument()
    await userEvent.click(d.getByRole('button', { name: 'Actualiser' }))
    await userEvent.click(d.getByRole('button', { name: 'Utiliser sa couverture' }))
    expect(await screen.findByText('Couverture changée')).toBeInTheDocument()
    fail = true
    await userEvent.click(d.getByRole('button', { name: 'Actualiser' }))
    expect(await screen.findByText('Action non faite')).toBeInTheDocument()
    await userEvent.click(d.getByRole('button', { name: 'Délier' }))
    await waitFor(() => expect(d.queryByText(/Liée à une fiche/)).not.toBeInTheDocument())
    expect(calls('POST').map(([url]) => url)).toEqual(['/api/series/7/metadata/refresh', '/api/series/7/metadata/cover', '/api/series/7/metadata/refresh'])
    expect(calls('POST')[1][1]).toEqual({ source: 'mangadex' })
    expect(calls('DELETE')).toEqual([['/api/series/7/metadata/links/mangadex', null]])
  })
})

describe('source principale', () => {
  it('se choisit parmi les sources liées, et revient à l’ordre réglé', async () => {
    const linked = { ...base, sources: [state({ external_id: '1', matched_by: 'auto' }), state({ source: 'mangadex', candidates: [fiche('x', 'X')] })] }
    const { calls, set } = serve(linked)
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Modifier' }))
    let d = within(await screen.findByRole('dialog'))
    const select = d.getByRole('combobox', { name: 'Source principale' })
    expect(select).toHaveTextContent('Automatique (ordre réglé)')
    expect(select).toHaveAccessibleDescription(/pour cette série seulement/)
    await userEvent.click(select)
    // Seule la source liée est proposée.
    expect(screen.queryByRole('option', { name: 'MangaDex' })).not.toBeInTheDocument()
    await userEvent.click(await screen.findByRole('option', { name: 'AniList' }))
    await userEvent.click(d.getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(calls('PATCH').at(-1)?.[1]).toEqual({ main_source: 'anilist' })

    set({ ...linked, metadata: { ...NO_METADATA, main_source: 'anilist' } })
    await userEvent.click(main().getByRole('button', { name: 'Modifier' }))
    d = within(await screen.findByRole('dialog'))
    await userEvent.click(d.getByRole('combobox', { name: 'Source principale' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Automatique (ordre réglé)' }))
    await userEvent.click(d.getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(calls('PATCH').at(-1)?.[1]).toEqual({ unlock: ['main_source'] }))
  })
})
