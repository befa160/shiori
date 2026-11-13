import { screen, waitFor, within } from '@testing-library/react'
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
  resume_volume_id: 'b',
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

describe('page d’une série', () => {
  const main = () => within(screen.getByRole('main'))
  const card = (name: string) => main().getByRole('link', { name: new RegExp(`^${name}`) }).closest('li') as HTMLElement

  it('montre la série, sa progression, le tome à reprendre et chaque tome dans l’ordre', async () => {
    mockApi({ '/api/series/7': () => json(detail) })
    const { router } = renderAt('/series/7')
    expect(await main().findByRole('heading', { level: 1, name: 'Toriko' })).toBeInTheDocument()
    expect(main().getByText('3 tomes · 1 tome lu sur 3')).toBeInTheDocument()
    expect(main().getByRole('progressbar', { name: '1 tome lu sur 3' })).toBeInTheDocument()
    expect(main().getByRole('link', { name: 'Reprendre · Tome 2' })).toHaveAttribute('href', '/read/b')
    expect(main().getByRole('combobox', { name: 'Statut de publication' })).toHaveTextContent('Statut inconnu')

    const titles = within(main().getByRole('region', { name: 'Tomes' }))
      .getAllByRole('listitem')
      .map((li) => within(li).getAllByTitle(/./)[0].textContent)
    expect(titles).toEqual(['Tome 1', 'Tome 2', 'bonus.cbr'])
    expect(card('Tome 1')).toHaveAttribute('id', 'volume-a')
    expect(card('Tome 1')).toHaveTextContent('Lu')
    expect(within(card('Tome 2')).getByRole('progressbar', { name: 'Progression de Tome 2' })).toBeInTheDocument()
    expect(card('Tome 2')).toHaveTextContent('Avertissement')
    expect(card('Tome 2')).toHaveTextContent('45 / 180')
    expect(card('bonus.cbr')).toHaveTextContent('Erreur')
    expect(card('bonus.cbr')).toHaveTextContent('12 pages')
    expect(main().getByRole('link', { name: /^Tome 1/ })).toHaveAttribute('href', '/read/a')
    // Sans la liste des bibliothèques (non fournie ici), le retour s'appelle « Bibliothèque ».
    await userEvent.click(main().getByRole('link', { name: 'Bibliothèque' }))
    expect(router.state.location.pathname).toBe('/libraries/1')
  })

  it('propose de relire une série lue, de lire une série jamais ouverte', async () => {
    const allRead = detail.volumes.map((v) => ({ ...v, progress: { page_id: 'x', ordinal: 0, page_count: 1, finished: true, updated_at: '' } }))
    mockApi({ '/api/series/7': () => json({ ...detail, resume_volume_id: 'a', volumes: allRead }) })
    const { unmount } = renderAt('/series/7')
    expect(await main().findByRole('link', { name: 'Relire · Tome 1' })).toBeInTheDocument()
    unmount()
    const fresh = detail.volumes.map((v) => ({ ...v, progress: undefined }))
    mockApi({ '/api/series/7': () => json({ ...detail, resume_volume_id: 'a', volumes: fresh }) })
    renderAt('/series/7')
    expect(await main().findByRole('link', { name: 'Lire · Tome 1' })).toBeInTheDocument()
  })

  it('change le statut de publication, et le dit si le serveur refuse', async () => {
    let fail = false
    const fetch = mockApi({
      '/api/series/7': (init) => {
        if (init?.method !== 'PATCH') return json(detail)
        if (fail) return json({ code: 'internal', message: 'erreur' }, 500)
        return json({ ...detail, ...JSON.parse(String(init.body)) })
      },
    })
    renderAt('/series/7')
    const select = await main().findByRole('combobox', { name: 'Statut de publication' })
    await userEvent.click(select)
    await userEvent.click(await screen.findByRole('option', { name: 'En pause' }))
    await waitFor(() => expect(select).toHaveTextContent('En pause'))
    const patch = fetch.mock.calls.find(([, init]) => init?.method === 'PATCH')
    expect(JSON.parse(String(patch?.[1]?.body))).toEqual({ publication_status: 'hiatus' })

    fail = true
    await userEvent.click(select)
    await userEvent.click(await screen.findByRole('option', { name: 'Statut inconnu' }))
    expect(await screen.findByText('Statut non enregistré')).toBeInTheDocument()
    expect(select).toHaveTextContent('En pause')
  })

  it('marque un tome lu (dernière page) ou non lu, depuis son menu', async () => {
    const fetch = mockApi({
      '/api/series/7': () => json(detail),
      '/api/volumes/c/pages': () => json([{ id: 'c1' }, { id: 'c12' }]),
      '/api/volumes/c/progress': () => json({ page_id: 'c12', ordinal: 1, page_count: 2, finished: true, updated_at: '' }),
      '/api/volumes/a/progress': () => new Response(null, { status: 204 }),
    })
    renderAt('/series/7')
    await main().findByRole('heading', { level: 1, name: 'Toriko' })
    // Un tome jamais ouvert : rien à effacer.
    await userEvent.click(main().getByRole('button', { name: 'Actions pour bonus.cbr' }))
    expect(screen.queryByRole('menuitem', { name: 'Marquer comme non lu' })).toBeNull()
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme lu' }))
    await waitFor(() =>
      expect(fetch.mock.calls.find(([, init]) => init?.method === 'PUT')).toEqual([
        '/api/volumes/c/progress',
        expect.objectContaining({ body: JSON.stringify({ page_id: 'c12' }) }),
      ]),
    )
    // Un tome lu : rien à marquer comme lu.
    await userEvent.click(main().getByRole('button', { name: 'Actions pour Tome 1' }))
    expect(screen.queryByRole('menuitem', { name: 'Marquer comme lu' })).toBeNull()
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' }))
    await waitFor(() => expect(fetch.mock.calls.some(([url, init]) => url === '/api/volumes/a/progress' && init?.method === 'DELETE')).toBe(true))
  })

  it('le dit quand un tome ne peut pas être modifié', async () => {
    mockApi({ '/api/series/7': () => json(detail), '/api/volumes/a/progress': () => json({ code: 'internal', message: 'x' }, 500) })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour Tome 1' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' }))
    expect(await screen.findByText('Tome non modifié')).toBeInTheDocument()
  })

  it('montre les problèmes du scan d’un tome, traduits d’après leur code', async () => {
    mockApi({
      '/api/series/7': () => json(detail),
      '/api/volumes/c': () =>
        json({
          issues: [
            { code: 'no_pages', severity: 'error', entry: null, message: 'aucune page' },
            { code: 'nouveau_code', severity: 'warning', entry: 'notes.txt', message: 'message du serveur' },
          ],
        }),
      '/api/volumes/a': () => json({ issues: [] }),
    })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour bonus.cbr' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Problèmes du scan' }))
    const dialog = await screen.findByRole('dialog', { name: 'Problèmes du scan · bonus.cbr' })
    expect(dialog).toHaveTextContent('Relevés au dernier scan de bonus.cbr.')
    expect(await within(dialog).findByText('Erreur · Aucune page lisible dans le fichier')).toHaveClass('text-destructive')
    // Code inconnu de l'interface : le message du serveur.
    expect(within(dialog).getByText('Avertissement · message du serveur')).toBeInTheDocument()
    expect(within(dialog).getByText('notes.txt')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'Fermer' }))
    expect(screen.queryByRole('dialog')).toBeNull()

    await userEvent.click(main().getByRole('button', { name: 'Actions pour Tome 1' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Problèmes du scan' }))
    expect(await screen.findByText('Aucun problème relevé.')).toBeInTheDocument()
  })

  it('signale une série sans tome', async () => {
    mockApi({ '/api/series/7': () => json({ ...detail, volumes: [], resume_volume_id: null }) })
    renderAt('/series/7')
    expect(await screen.findByText('Aucun tome dans cette série.')).toBeInTheDocument()
    expect(screen.queryByRole('progressbar')).toBeNull()
  })

  it('revient à l’accueil si la série est inconnue', async () => {
    mockApi({ '/api/series/9': () => json({ code: 'not_found', message: 'série introuvable' }, 404) })
    renderAt('/series/9')
    expect(await screen.findByRole('alert')).toHaveTextContent('Série introuvable')
    expect(main().getByRole('link', { name: 'Accueil' })).toHaveAttribute('href', '/')
  })
})

describe('identifiant invalide dans l’adresse', () => {
  it.each(['/series/abc', '/libraries/0', '/series/1.5'])('%s : page introuvable, sans appeler l’API', async (path) => {
    const fetch = mockApi({})
    renderAt(path)
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    // Seuls les réglages (langue), les bibliothèques (sidebar) et le scan en cours (suivi) sont
    // demandés, jamais la ressource.
    expect(new Set(fetch.mock.calls.map((c) => c[0]))).toEqual(
      new Set(['/api/settings', '/api/libraries', '/api/admin/scans/current']),
    )
  })
})
