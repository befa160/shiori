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
  followed: false,
  to_read: false,
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
  unlinked: false,
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
    const sources = within(await d.findByRole('region', { name: 'Sources liées' }))
    expect(sources.getByRole('listitem')).toHaveTextContent(/AniList · choisie par vous/)
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
    // Les deux sources liées sont en tête, chacune avec ses boutons.
    const linked = {
      ...base,
      sources: [
        state({ external_id: '1', matched_by: 'user' }),
        state({ source: 'mangadex', external_id: 'abc', matched_by: 'auto', last_error: 'offline' }),
      ],
    }
    const unlinked = { ...base, sources: [linked.sources[0], state({ source: 'mangadex', unlinked: true })] }
    const after = { ...linked, metadata: { ...NO_METADATA, cover_image_id: 'c1' } }
    let fail = false
    const { calls } = serve(linked, {
      '/api/series/7/metadata/refresh': () => (fail ? json({ code: 'offline', message: 'x' }, 502) : json(linked)),
      '/api/series/7/metadata/cover': () => json(after),
      '/api/series/7/metadata/links/mangadex': () => json(unlinked),
    })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Identifier' }))
    const d = await dialog()
    const items = () => within(d.getByRole('region', { name: 'Sources liées' })).getAllByRole('listitem')
    expect(items().map((li) => li.textContent)).toEqual([
      expect.stringMatching(/^AniList · choisie par vous/),
      expect.stringMatching(/^MangaDex · trouvée automatiquement/),
    ])
    // Les boutons de chaque source sont groupés sous son nom (lecteur d'écran).
    expect(within(items()[1]).getByRole('group', { name: 'MangaDex' })).toBeInTheDocument()
    const mangadex = within(items()[1])
    expect(mangadex.getByText('Dernier essai : source injoignable')).toBeInTheDocument()
    await userEvent.click(mangadex.getByRole('button', { name: 'Actualiser' }))
    await userEvent.click(mangadex.getByRole('button', { name: 'Utiliser sa couverture' }))
    expect(await screen.findByText('Couverture changée')).toBeInTheDocument()
    fail = true
    await userEvent.click(within(items()[1]).getByRole('button', { name: 'Actualiser' }))
    expect(await screen.findByText('Action non faite')).toBeInTheDocument()
    await userEvent.click(within(items()[1]).getByRole('button', { name: 'Délier' }))
    // Déliée : elle quitte la liste, et la fenêtre dit qu'elle ne sera plus reliée seule.
    await waitFor(() => expect(items()).toHaveLength(1))
    expect(d.getByText(/MangaDex : déliée par vous/)).toBeInTheDocument()
    expect(calls('POST').map(([url]) => url)).toEqual(['/api/series/7/metadata/refresh', '/api/series/7/metadata/cover', '/api/series/7/metadata/refresh'])
    expect(calls('POST')[1][1]).toEqual({ source: 'mangadex' })
    expect(calls('DELETE')).toEqual([['/api/series/7/metadata/links/mangadex', null]])
  })

  it('montre une source Wikipédia : lien vers l’article, pas de couverture, licence créditée', async () => {
    serve({ ...base, sources: [state({ source: 'wikipedia', external_id: 'fr:Toriko (manga)', matched_by: 'auto' })] })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Identifier' }))
    const d = await dialog()
    const item = within(within(d.getByRole('region', { name: 'Sources liées' })).getByRole('listitem'))
    expect(item.getByRole('link', { name: 'Voir la fiche' })).toHaveAttribute('href', 'https://fr.wikipedia.org/wiki/Toriko_(manga)')
    expect(item.queryByRole('button', { name: 'Utiliser sa couverture' })).not.toBeInTheDocument()
    expect(item.getByRole('button', { name: 'Délier' })).toBeInTheDocument()
    expect(d.getByText(/textes sous licence CC BY-SA/)).toBeInTheDocument()
    expect(d.getByRole('link', { name: 'Wikipédia' })).toHaveAttribute('href', 'https://www.wikipedia.org')
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
