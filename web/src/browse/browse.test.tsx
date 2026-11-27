import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, SeriesDetail, SeriesSummary, VolumeMetadata } from '@/api/types'
import { json, mockApi, renderAt, NO_METADATA, READER } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const library: Library = { id: 1, name: 'Mangas', root: '/m', kind: 'manga', watch: true, series_count: 2, volume_count: 1 }
const longName =
  'Une série au titre extrêmement long, choisi pour vérifier que la troncature sur deux lignes tient sur tous les écrans'
const summary = {
  library_id: 1,
  read_count: 0,
  page_count: 30,
  pages_read: 0,
  reading_status: 'unread',
  publication_status: null,
  created_at: '2026-11-12T00:00:00.000Z',
  last_read_at: null, to_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
  cover_image_id: null,
  alt_titles: [] as string[],
  authors: [] as string[],
  tags: [] as string[],
} as const
const series: SeriesSummary[] = [
  { ...summary, id: 7, name: 'Toriko', volume_count: 1 },
  { ...summary, id: 8, name: longName, volume_count: 3 },
]
const settings = { reading_mode: 'rtl', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null } as const
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
  collections: [],
  resume_volume_id: 'b',
  page_count: 392,
  pages_read: 245,
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
    const reading = { ...series[1], reading_status: 'in_progress', read_count: 1, pages_read: 10 } as const
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
    expect(within(long).getByRole('progressbar', { name: /^33\s%\slu$/ })).toHaveAttribute('aria-valuenow', '33')
    // Sans couverture : couverture générée ; sans tome à reprendre : pas de bouton « Lire ».
    expect(within(long).getByTestId('fallback-cover')).toHaveTextContent(longName)
    expect(main.queryByRole('link', { name: `Lire ${longName}` })).toBeNull()
  })

  it('n’affiche « 100 % » que pour une série finie', async () => {
    const almost = { ...series[0], reading_status: 'in_progress', page_count: 12_760, pages_read: 12_700 } as const
    mockApi({ '/api/libraries': () => json([library]), '/api/libraries/1/series': () => json([almost]) })
    renderAt('/libraries/1')
    expect(await within(screen.getByRole('main')).findByRole('progressbar', { name: /^99\s%\slu$/ })).toBeInTheDocument()
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
    // Titre venu du ComicInfo du tome 2, après son numéro.
    const titled = { ...detail, volumes: detail.volumes.map((v) => (v.volume_id === 'b' ? { ...v, title: 'Le Gourmet' } : v)) }
    mockApi({ '/api/series/7': () => json(titled) })
    const { router } = renderAt('/series/7')
    expect(await main().findByRole('heading', { level: 1, name: 'Toriko' })).toBeInTheDocument()
    // En pages : tome 1 fini (200), tome 2 à la page 45 (sur 180), tome 3 jamais ouvert (12) : 245 / 392, 62,5 % arrondi vers le bas.
    expect(main().getByText(/^3 tomes · 1 tome lu sur 3 · 62\s%$/)).toBeInTheDocument()
    expect(main().getByRole('progressbar', { name: /^62\s%\slu$/ })).toBeInTheDocument()
    expect(main().getByRole('link', { name: 'Reprendre · Tome 2' })).toHaveAttribute('href', '/read/b')

    const titles = within(main().getByRole('region', { name: 'Tomes' }))
      .getAllByRole('listitem')
      .map((li) => within(li).getAllByTitle(/./)[0].textContent)
    expect(titles).toEqual(['Tome 1', 'Tome 2 · Le Gourmet', 'bonus.cbr'])
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

  it('montre « Avertissement » et « Lu » ensemble sur un tome lu qui a un problème de scan', async () => {
    const warned = { ...detail.volumes[1], progress: { page_id: 'q', ordinal: 179, page_count: 180, finished: true, updated_at: '' } }
    mockApi({ '/api/series/7': () => json({ ...detail, volumes: [warned] }) })
    renderAt('/series/7')
    const tome = (await main().findByRole('link', { name: /^Tome 2/ })).closest('li') as HTMLElement
    expect(within(tome).getByText('Avertissement')).toBeInTheDocument()
    expect(within(tome).getByText('Lu')).toBeInTheDocument()
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

  it('montre les métadonnées : auteurs, repères, résumé repliable, genres, autres titres, liens web seulement', async () => {
    const metadata = {
      ...NO_METADATA,
      authors: [{ name: 'Mitsutoshi Shimabukuro', story: true, art: true }],
      year: 2008,
      publisher: 'Kana',
      publication_status: 'completed' as const,
      summary: 'Toriko, chasseur gourmet, parcourt le monde. '.repeat(10),
      genres: ['Action', 'Aventure'],
      tags: ['Aventure', 'Cuisine'],
      alt_titles: ['トリコ'],
      links: ['https://example.org/toriko', 'javascript:alert(1)', 'pas une adresse'],
    }
    mockApi({ '/api/series/7': () => json({ ...detail, metadata }) })
    renderAt('/series/7')
    expect(await main().findByText('Mitsutoshi Shimabukuro', { selector: 'p' })).toBeInTheDocument()
    expect(main().getByText('2008 · Kana · Publication terminée')).toBeInTheDocument()
    const chips = within(main().getByRole('list', { name: 'Genres et tags' })).getAllByRole('listitem')
    expect(chips.map((li) => li.textContent)).toEqual(['Action', 'Aventure', 'Cuisine'])
    // Chaque tag mène aux séries qui l'ont.
    // Le tag affiché en français mène à son mot de source (« Cuisine » → `Cooking`).
    expect(within(chips[2]).getByRole('link', { name: 'Cuisine' })).toHaveAttribute('href', '/series?tag=Cooking')
    // Les autres titres servent à la recherche, ils ne s'affichent pas (trop nombreux sur les vraies fiches).
    expect(main().queryByText(/トリコ/)).not.toBeInTheDocument()
    const links = main().getAllByRole('link', { name: /example\.org/ })
    expect(links.map((a) => a.getAttribute('href'))).toEqual(['https://example.org/toriko'])
    expect(links[0]).toHaveAttribute('rel', 'noreferrer noopener')
    expect(main().queryByRole('link', { name: /alert/ })).not.toBeInTheDocument()
    const more = main().getByRole('button', { name: 'Lire la suite' })
    expect(more).toHaveAttribute('aria-expanded', 'false')
    await userEvent.click(more)
    expect(main().getByRole('button', { name: 'Réduire' })).toHaveAttribute('aria-expanded', 'true')
  })

  it('modifie une série : seuls les champs changés partent, « Rétablir » rend un champ aux fichiers, un refus se dit', async () => {
    let fail = false
    const oda = { name: 'Oda', story: true, art: true }
    const metadata = { ...NO_METADATA, summary: 'Mon résumé', year: 2008, authors: [oda] }
    const fetch = mockApi({
      '/api/series/7': (init) => {
        if (init?.method !== 'PATCH') return json({ ...detail, metadata, locked: ['summary'] })
        if (fail) return json({ code: 'invalid_body', message: 'refusé' }, 400)
        const patch = JSON.parse(String(init.body))
        if (patch.unlock) return json({ ...detail, metadata: { ...metadata, summary: 'Résumé des fichiers' }, locked: [] })
        return json({ ...detail, name: patch.title ?? detail.name, metadata: { ...metadata, ...patch }, locked: ['summary', ...Object.keys(patch)] })
      },
    })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Modifier' }))
    const dialog = within(await screen.findByRole('dialog', { name: 'Modifier « Toriko »' }))
    const patches = () => fetch.mock.calls.filter(([, init]) => init?.method === 'PATCH').map(([, init]) => JSON.parse(String(init?.body)))

    // Un champ modifié à la main se signale et se rétablit.
    expect(dialog.getByText('Modifié à la main')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Rendre « Résumé » aux fichiers' }))
    await waitFor(() => expect(dialog.getByLabelText('Résumé')).toHaveValue('Résumé des fichiers'))
    expect(patches()).toEqual([{ unlock: ['summary'] }])

    // Année invalide : l'erreur est sous le champ et l'enregistrement attend.
    await userEvent.type(dialog.getByLabelText('Titre'), '  Toriko (Kana)  ')
    await userEvent.clear(dialog.getByLabelText('Année'))
    await userEvent.type(dialog.getByLabelText('Année'), 'vingt')
    expect(dialog.getByRole('alert')).toHaveTextContent('Une année entre 1 et 9999.')
    expect(dialog.getByLabelText('Année')).toHaveAttribute('aria-invalid', 'true')
    expect(dialog.getByRole('button', { name: 'Enregistrer' })).toBeDisabled()

    fail = true
    await userEvent.clear(dialog.getByLabelText('Année'))
    await userEvent.type(dialog.getByLabelText('Année'), '2009')
    await userEvent.click(dialog.getByRole('button', { name: 'Enregistrer' }))
    expect(await dialog.findByRole('alert')).toHaveTextContent('Modifications non enregistrées')

    fail = false
    // Listes : une par ligne (un auteur connu garde ses rôles), genres séparés par des virgules ;
    // statut choisi dans la liste.
    await userEvent.type(dialog.getByLabelText('Auteurs'), '{Enter}Nouvel auteur{Enter}')
    await userEvent.type(dialog.getByLabelText('Genres'), 'Action, , Cuisine ')
    await userEvent.type(dialog.getByLabelText('Liens'), 'https://example.org')
    await userEvent.type(dialog.getByLabelText('Autres titres'), 'トリコ')
    await userEvent.type(dialog.getByLabelText('Éditeur'), '   ')
    await userEvent.click(dialog.getByLabelText('Statut de publication'))
    await userEvent.click(await screen.findByRole('option', { name: 'En pause' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(patches().at(-1)).toEqual({
      title: 'Toriko (Kana)',
      year: 2009,
      authors: [oda, { name: 'Nouvel auteur', story: false, art: false }],
      genres: ['Action', 'Cuisine'],
      links: ['https://example.org'],
      alt_titles: ['トリコ'],
      publisher: null,
      publication_status: 'hiatus',
    })
    expect(await screen.findByText('Modifications enregistrées')).toBeInTheDocument()
    expect(main().getByRole('heading', { level: 1, name: 'Toriko (Kana)' })).toBeInTheDocument()

    // Rien de changé : la fenêtre se ferme sans requête.
    const before = patches().length
    await userEvent.click(main().getByRole('button', { name: 'Modifier' }))
    await userEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(patches()).toHaveLength(before)
    // Beaucoup de saisie : plus de 5 s quand toute la suite tourne en parallèle.
  }, 15_000)

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
        expect.objectContaining({ body: JSON.stringify({ page_id: 'c12', stats: false }) }),
      ]),
    )
    // Un tome lu : rien à marquer comme lu.
    await userEvent.click(main().getByRole('button', { name: 'Actions pour Tome 1' }))
    expect(screen.queryByRole('menuitem', { name: 'Marquer comme lu' })).toBeNull()
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' }))
    await waitFor(() => expect(fetch.mock.calls.some(([url, init]) => url === '/api/volumes/a/progress' && init?.method === 'DELETE')).toBe(true))
  })

  it('marque toute la série lue, ou non lue après confirmation, depuis son menu', async () => {
    const read = { ...detail, volumes: detail.volumes.map((v) => ({ ...v, progress: { page_id: 'z', ordinal: v.page_count - 1, page_count: v.page_count, finished: true, updated_at: '' } })) }
    let answer: SeriesDetail = read
    const fetch = mockApi({ '/api/series/7': () => json(detail), '/api/series/7/read': () => json(answer) })
    renderAt('/series/7')
    await main().findByRole('heading', { level: 1, name: 'Toriko' })
    // Commencée : les deux actions.
    await userEvent.click(main().getByRole('button', { name: 'Actions pour Toriko' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme lu' }))
    await waitFor(() => expect(fetch.mock.calls.some(([url, init]) => url === '/api/series/7/read' && init?.method === 'PUT')).toBe(true))
    // Lue : seulement « non lu », qui demande confirmation ; annuler ne change rien.
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour Toriko' }))
    expect(screen.queryByRole('menuitem', { name: 'Marquer comme lu' })).toBeNull()
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' }))
    const confirm = await screen.findByRole('alertdialog', { name: 'Marquer « Toriko » comme non lue ?' })
    expect(confirm).toHaveTextContent('La progression de ses 3 tomes sera effacée, pour votre compte seulement.')
    await userEvent.click(within(confirm).getByRole('button', { name: 'Annuler' }))
    expect(fetch.mock.calls.some(([, init]) => init?.method === 'DELETE')).toBe(false)
    answer = { ...detail, volumes: detail.volumes.map((v) => ({ ...v, progress: undefined })) }
    await userEvent.click(main().getByRole('button', { name: 'Actions pour Toriko' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Marquer comme non lu' }))
    await waitFor(() => expect(fetch.mock.calls.some(([url, init]) => url === '/api/series/7/read' && init?.method === 'DELETE')).toBe(true))
  })

  it('suit la série puis ne la suit plus ; un refus se dit et ne change rien', async () => {
    let status = 204
    const fetch = mockApi({
      '/api/series/7': () => json(detail),
      '/api/series/7/follow': () => (status === 204 ? new Response(null, { status }) : json({ code: 'internal', message: 'x' }, status)),
    })
    renderAt('/series/7')
    const follow = await main().findByRole('button', { name: 'Suivre' })
    expect(follow).toHaveAttribute('aria-pressed', 'false')
    // Commencée : pas de bouton « À lire » (elle en sortirait aussitôt).
    expect(main().queryByRole('button', { name: 'À lire' })).toBeNull()
    await userEvent.click(follow)
    await waitFor(() => expect(follow).toHaveAttribute('aria-pressed', 'true'))
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/series/7/follow' && init?.method === 'PUT')).toBe(true)
    expect(await screen.findByText('Série suivie : ses nouveaux tomes s’afficheront à l’accueil')).toBeInTheDocument()
    await userEvent.click(follow)
    await waitFor(() => expect(follow).toHaveAttribute('aria-pressed', 'false'))
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/series/7/follow' && init?.method === 'DELETE')).toBe(true)
    status = 500
    await userEvent.click(follow)
    expect(await screen.findByText('Suivi non modifié')).toBeInTheDocument()
    expect(follow).toHaveAttribute('aria-pressed', 'false')
  })

  it('met une série pas commencée dans « À lire », puis l’en retire', async () => {
    const fresh = { ...detail, volumes: detail.volumes.map((v) => ({ ...v, progress: undefined })) }
    const fetch = mockApi({ '/api/series/7': () => json(fresh), '/api/series/7/to-read': () => new Response(null, { status: 204 }) })
    renderAt('/series/7')
    const button = await main().findByRole('button', { name: 'À lire' })
    expect(button).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(button)
    await waitFor(() => expect(button).toHaveAttribute('aria-pressed', 'true'))
    expect(await screen.findByText('Série ajoutée à « À lire »')).toBeInTheDocument()
    await userEvent.click(button)
    await waitFor(() => expect(button).toHaveAttribute('aria-pressed', 'false'))
    expect(fetch.mock.calls.filter(([url]) => url === '/api/series/7/to-read').map(([, init]) => init?.method)).toEqual(['PUT', 'DELETE'])
  })

  it('compte une série lue quand tout ce qui se lit est lu, malgré un tome illisible', async () => {
    const done = { page_id: 'z', ordinal: 0, page_count: 1, finished: true, updated_at: '' }
    const volumes = [
      { ...detail.volumes[0], progress: done },
      { ...detail.volumes[2], page_count: 0, progress: undefined },
    ]
    mockApi({ '/api/series/7': () => json({ ...detail, volumes }) })
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour Toriko' }))
    expect(await screen.findByRole('menuitem', { name: 'Marquer comme non lu' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'Marquer comme lu' })).toBeNull()
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

  it('modifie un tome : titre envoyé seulement s’il change, titre et couverture rendus aux fichiers', async () => {
    const meta: VolumeMetadata = { title: 'Le gourmet', number: null, summary: null, release_date: null, authors: [], cover_page_id: 'p2' }
    let volume = { id: 'a', metadata: meta, locked: ['title', 'cover_page_id'] }
    const fetch = mockApi({
      '/api/series/7': () => json(detail),
      '/api/volumes/a': (init?: RequestInit) => {
        const patch = init?.method === 'PATCH' ? JSON.parse(String(init.body)) : null
        if (patch?.unlock?.includes('title')) volume = { id: 'a', metadata: { ...meta, title: 'Du fichier' }, locked: ['cover_page_id'] }
        if (patch?.unlock?.includes('cover_page_id')) volume = { id: 'a', metadata: { ...volume.metadata, cover_page_id: null }, locked: [] }
        if (patch?.title === null) return json({ code: 'invalid_body', message: 'refusé' }, 400)
        return json(volume)
      },
    })
    const patches = () =>
      fetch.mock.calls.filter(([url, init]) => url === '/api/volumes/a' && init?.method === 'PATCH').map(([, init]) => JSON.parse(String(init?.body)))
    renderAt('/series/7')
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour Tome 1' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Modifier' }))
    const dialog = await screen.findByRole('dialog', { name: 'Modifier « Tome 1 »' })
    const title = await within(dialog).findByRole('textbox', { name: 'Titre' })
    expect(title).toHaveValue('Le gourmet')
    // Rien de changé : rien n'est envoyé.
    await userEvent.click(within(dialog).getByRole('button', { name: 'Enregistrer' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(patches()).toEqual([])

    await userEvent.click(main().getByRole('button', { name: 'Actions pour Tome 1' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Modifier' }))
    const again = await screen.findByRole('dialog', { name: 'Modifier « Tome 1 »' })
    await userEvent.click(await within(again).findByRole('button', { name: 'Rendre « Titre » aux fichiers' }))
    await waitFor(() => expect(within(again).getByRole('textbox', { name: 'Titre' })).toHaveValue('Du fichier'))
    await userEvent.click(within(again).getByRole('button', { name: 'Rendre « Couverture » aux fichiers' }))
    expect(await screen.findByText('Couverture du tome rendue à sa première page')).toBeInTheDocument()
    // Un titre vidé part en `null` ; le refus du serveur se dit dans la fenêtre.
    await userEvent.clear(within(again).getByRole('textbox', { name: 'Titre' }))
    await userEvent.click(within(again).getByRole('button', { name: 'Enregistrer' }))
    expect(await within(again).findByRole('alert')).toHaveTextContent('Modifications non enregistrées')
    await userEvent.type(within(again).getByRole('textbox', { name: 'Titre' }), 'Nouveau')
    await userEvent.click(within(again).getByRole('button', { name: 'Enregistrer' }))
    // Enregistré : la fenêtre se ferme (le toast d'un test précédent peut encore être affiché).
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(patches()).toEqual([{ unlock: ['title'] }, { unlock: ['cover_page_id'] }, { title: null }, { title: 'Nouveau' }])
  })

  it('un lecteur ne voit pas « Modifier » dans le menu d’un tome', async () => {
    mockApi({ '/api/series/7': () => json(detail) })
    renderAt('/series/7', { auth: { setup_required: false, user: READER } })
    await userEvent.click(await main().findByRole('button', { name: 'Actions pour Tome 1' }))
    await screen.findByRole('menuitem', { name: 'Problèmes du scan' })
    expect(screen.queryByRole('menuitem', { name: 'Modifier' })).toBeNull()
  })

  it('montre les marque-pages de la série, repliés, dans l’ordre de lecture', async () => {
    const mark = (id: number, volume_id: string, ordinal: number, note = '') => ({ id, volume_id, page_id: `${volume_id}${ordinal}`, ordinal, note, created_at: '' })
    mockApi({
      '/api/series/7': () => json(detail),
      // Rangés par identifiant de tome côté serveur ; un tome sorti de la série est ignoré.
      '/api/series/7/bookmarks': () => json([mark(1, 'b', 9), mark(2, 'a', 50, 'Le gourmet'), mark(3, 'a', 4), mark(4, 'z', 1)]),
    })
    renderAt('/series/7')
    const toggle = await main().findByRole('button', { name: 'Marque-pages (3)' })
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await userEvent.click(toggle)
    const links = within(main().getByRole('list', { name: 'Marque-pages (3)' })).getAllByRole('link')
    expect(links.map((l) => [l.textContent, l.getAttribute('href')])).toEqual([
      ['Tome 1 · page 5', '/read/a?page=a4'],
      ['Tome 1 · page 51Le gourmet', '/read/a?page=a50'],
      ['Tome 2 · page 10', '/read/b?page=b9'],
    ])
  })

  it('sans marque-page, pas de section', async () => {
    mockApi({ '/api/series/7': () => json(detail), '/api/series/7/bookmarks': () => json([]) })
    renderAt('/series/7')
    await main().findByRole('heading', { name: 'Tomes' })
    expect(main().queryByRole('button', { name: /^Marque-page/ })).toBeNull()
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
    // Seuls les réglages et les préférences (langue), les bibliothèques (sidebar) et le scan en
    // cours (suivi) sont demandés, jamais la ressource.
    expect(new Set(fetch.mock.calls.map((c) => c[0]))).toEqual(
      new Set(['/api/settings', '/api/me', '/api/libraries', '/api/admin/scans/current']),
    )
  })
})
