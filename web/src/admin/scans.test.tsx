import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, LogLine, RunSummary, ScanProgress } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const libraries: Library[] = [
  { id: 1, name: 'Mangas', root: '/srv/manga', kind: 'manga', series_count: 1, volume_count: 2 },
  { id: 2, name: 'Comics', root: '/srv/comics', kind: 'comics', series_count: 0, volume_count: 0 },
]
const counts = { ok: 3, warning: 1, error: 2, unchanged: 4, removed: 0 }
const progress: ScanProgress = { run: 9, library_id: 1, target: '/srv/manga', total: 10, done: 3, current: '/srv/manga/A/A T01.cbz', counts, cancelling: false }
const run = (id: number, over: Partial<RunSummary> = {}): RunSummary => ({
  id,
  library_id: 1,
  origin: 'api',
  status: 'completed',
  root: '/srv/manga',
  options: { deep: false, rehash: false },
  started_at: '2026-11-13T09:00:00.000Z',
  finished_at: '2026-11-13T09:00:12.000Z',
  duration_ms: 12_480,
  files_total: 10,
  counts,
  error: null,
  ...over,
})
const none = () => new Response(null, { status: 204 })
const main = () => within(screen.getByRole('main'))
const urls = (fetch: ReturnType<typeof mockApi>) => fetch.mock.calls.map(([url]) => String(url))

describe('passe en cours', () => {
  it('suit la progression, puis rafraîchit les listes et dit comment le scan s’est terminé', async () => {
    let running = true
    const fetch = mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': () => (running ? json(progress) : none()),
      '/api/admin/scans/9': () => json(run(9)),
    })
    renderAt('/admin')
    const card = within((await main().findByRole('heading', { name: 'Scan de « Mangas »' })).closest('div.rounded-md') as HTMLElement)
    expect(card.getByText('3 fichiers sur 10')).toBeInTheDocument()
    expect(card.getByRole('progressbar', { name: 'Avancement du scan' })).toBeInTheDocument()
    expect(card.getByText('/srv/manga/A/A T01.cbz')).toBeInTheDocument()
    expect(card.getByText('Erreurs').nextSibling).toHaveClass('text-destructive')
    const before = urls(fetch).filter((u) => u === '/api/libraries').length
    running = false
    expect(await screen.findByText('Scan terminé', {}, { timeout: 3000 })).toBeInTheDocument()
    expect(screen.getByText('Sans problème : 3 · Avertissements : 1 · Erreurs : 2')).toBeInTheDocument()
    expect(await main().findByText('Aucun scan en cours.')).toBeInTheDocument()
    await waitFor(() => expect(urls(fetch).filter((u) => u === '/api/libraries').length).toBeGreaterThan(before))
  })

  it('annule le scan, et le montre pendant l’annulation', async () => {
    let cancelling = false
    const fetch = mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': () => json({ ...progress, total: 0, cancelling }),
      '/api/admin/scans/current/cancel': () => {
        cancelling = true
        return new Response(null, { status: 202 })
      },
    })
    renderAt('/admin')
    expect(await main().findByText('Liste des fichiers en préparation…')).toBeInTheDocument()
    await userEvent.click(main().getByRole('button', { name: 'Annuler le scan' }))
    expect(await main().findByRole('button', { name: 'Annulation…' })).toBeDisabled()
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/admin/scans/current/cancel' && init?.method === 'POST')).toBe(true)
  })
})

describe('scan très court', () => {
  it('annonce quand même sa fin et rafraîchit les listes', async () => {
    // Le serveur répond déjà « rien ne tourne » quand le suivi l'interroge après le lancement.
    const fetch = mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': none,
      '/api/admin/scans': () => json({ run: 12 }, 202),
      '/api/admin/scans/12': () => json(run(12)),
    })
    renderAt('/admin')
    await userEvent.click(await main().findByRole('button', { name: 'Scanner Mangas' }))
    expect(await screen.findByText('Scan terminé')).toBeInTheDocument()
    expect(urls(fetch)).toContain('/api/admin/scans/12')
  })
})

describe('scan lancé ailleurs', () => {
  it('apparaît sur la carte « Scan en cours » sans recharger', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    let running = false
    mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': () => (running ? json(progress) : none()),
    })
    renderAt('/admin')
    expect(await main().findByText('Aucun scan en cours.')).toBeInTheDocument()
    running = true
    await vi.advanceTimersByTimeAsync(10_000)
    expect(await main().findByRole('heading', { name: 'Scan de « Mangas »' })).toBeInTheDocument()
    vi.useRealTimers()
  })
})

describe('page des scans', () => {
  const history = (status = '') => `/api/admin/scans?limit=20${status}`

  it('lance un scan avec ses options, sauf si un scan tourne déjà', async () => {
    let running = false
    const fetch = mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': () => (running ? json(progress) : none()),
      [history()]: () => json([]),
      '/api/admin/scans': () => {
        running = true
        return json({ run: 9 }, 202)
      },
    })
    renderAt('/admin/scans')
    const form = within(await main().findByRole('form', { name: 'Lancer un scan' }))
    await userEvent.click(form.getByRole('combobox', { name: 'Bibliothèque' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Comics' }))
    await userEvent.click(form.getByRole('checkbox', { name: 'Contrôle approfondi' }))
    expect(form.getByRole('checkbox', { name: 'Relire tous les fichiers' })).toHaveAccessibleDescription(/Recalcule l'empreinte/)
    await userEvent.click(form.getByRole('button', { name: 'Scanner' }))
    const post = fetch.mock.calls.find(([url, init]) => url === '/api/admin/scans' && init?.method === 'POST')
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ library_id: 2, deep: true, rehash: false })
    expect(await form.findByText('Un scan est déjà en cours.')).toBeInTheDocument()
    expect(form.getByRole('button', { name: 'Scanner' })).toBeDisabled()
  })

  it('liste l’historique, le filtre par statut et charge la suite', async () => {
    const full = Array.from({ length: 20 }, (_, i) => run(40 - i))
    const fetch = mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': none,
      [history()]: () => json(full),
      '/api/admin/scans?limit=20&before=21': () => json([run(5, { status: 'interrupted', duration_ms: null })]),
      [history('&status=failed')]: () => json([run(3, { status: 'failed', library_id: null, root: '/ailleurs' })]),
    })
    const { router } = renderAt('/admin/scans')
    const region = within(await main().findByRole('region', { name: 'Historique' }))
    const first = (await region.findAllByRole('link'))[0]
    expect(first).toHaveAttribute('href', '/admin/scans/40')
    expect(first).toHaveTextContent('Mangas')
    expect(first).toHaveTextContent('Sans problème : 3 · Avertissements : 1 · Erreurs : 2')
    await userEvent.click(region.getByRole('button', { name: 'Charger la suite' }))
    expect(await region.findByText('Interrompu')).toBeInTheDocument()
    expect(region.queryByRole('button', { name: 'Charger la suite' })).toBeNull()

    await userEvent.click(region.getByRole('combobox', { name: 'Statut' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Échec' }))
    expect(router.state.location.search).toBe('?status=failed')
    const failed = (await region.findByText('/ailleurs')).closest('a') as HTMLElement
    expect(within(failed).getByText('Échec')).toHaveClass('text-destructive')
    expect(urls(fetch)).toContain(history('&status=failed'))
  })

  it('garde le filtre de bibliothèque retirable, même s’il ne reste qu’une bibliothèque', async () => {
    mockApi({
      '/api/libraries': () => json([libraries[0]]),
      '/api/admin/scans/current': none,
      [history('&library_id=2')]: () => json([]),
    })
    renderAt('/admin/scans?library=2')
    const region = within(await main().findByRole('region', { name: 'Historique' }))
    expect(await region.findByText('Aucun scan pour l\'instant.')).toBeInTheDocument()
    expect(region.getByRole('combobox', { name: 'Bibliothèque' })).toBeInTheDocument()
  })
})

describe('détail d’une passe', () => {
  const logs: LogLine[] = [
    { id: 1, at: '2026-11-13T09:00:01.000Z', level: 'info', path: '/srv/manga/A/A T01.cbz', code: null, message: 'scanné : ok, 12 page(s)', series_id: 7, volume_id: 'v1' },
    { id: 2, at: '2026-11-13T09:00:02.000Z', level: 'error', path: '/srv/manga/B.cbz', code: 'no_pages', message: 'aucune page', series_id: null, volume_id: null },
  ]

  it('montre les options, l’erreur fatale et le journal, traduit, avec un lien vers le tome', async () => {
    mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': none,
      '/api/admin/scans/9': () => json(run(9, { status: 'failed', options: { deep: true }, error: 'disque débranché' })),
      '/api/admin/scans/9/logs?level=info&limit=100': () => json(logs),
      '/api/admin/scans/9/logs?level=error&limit=100': () => json([logs[1]]),
    })
    const { router } = renderAt('/admin/scans/9')
    expect(await main().findByRole('heading', { level: 1, name: 'Scan n° 9' })).toBeInTheDocument()
    expect(await main().findByRole('alert')).toHaveTextContent('Erreur fatale : disque débranché')
    expect(main().getByText('Contrôle approfondi').nextSibling).toHaveTextContent('Oui')
    expect(main().getByText('Relire tous les fichiers').nextSibling).toHaveTextContent('Non')
    expect(main().getByText('Durée').nextSibling).toHaveTextContent(/12,5\ss/)
    const journal = within(main().getByRole('region', { name: 'Journal' }))
    expect(await journal.findByText(/scanné : ok, 12 page\(s\)/)).toBeInTheDocument()
    expect(journal.getByText(/Aucune page lisible dans le fichier/)).toBeInTheDocument()
    expect(journal.getByRole('link', { name: 'Voir le tome' })).toHaveAttribute('href', '/series/7#volume-v1')
    expect(journal.getAllByRole('link', { name: 'Voir le tome' })).toHaveLength(1)

    await userEvent.click(journal.getByRole('combobox', { name: 'Niveau' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Erreurs seulement' }))
    expect(router.state.location.search).toBe('?level=error')
    await waitFor(() => expect(journal.queryByText(/scanné : ok/)).toBeNull())
  })

  it('dit qu’une passe n’existe pas', async () => {
    mockApi({
      '/api/libraries': () => json(libraries),
      '/api/admin/scans/current': none,
      '/api/admin/scans/99': () => json({ code: 'not_found', message: 'passe introuvable' }, 404),
      '/api/admin/scans/99/logs?level=info&limit=100': () => json({ code: 'not_found', message: 'passe introuvable' }, 404),
    })
    renderAt('/admin/scans/99')
    expect((await main().findAllByRole('alert'))[0]).toHaveTextContent('Scan introuvable')
  })
})

describe('lien vers un tome', () => {
  it('fait défiler jusqu’au tome et lui donne le focus', async () => {
    const scroll = vi.fn()
    Element.prototype.scrollIntoView = scroll
    const volume = { kind: 'volume', number: 1, number_end: null, status: 'ok', page_count: 3, cover_page_id: null, file_name: 'A T01.cbz' } as const
    mockApi({
      '/api/admin/scans/current': none,
      '/api/series/7': () =>
        json({
          id: 7,
          library_id: 1,
          name: 'A',
          folder: 'A',
          publication_status: null,
          resume_volume_id: 'v1',
          settings: { reading_mode: 'rtl', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null },
          volumes: [
            { ...volume, volume_id: 'v0', number: 0 },
            { ...volume, volume_id: 'v1' },
          ],
        }),
    })
    renderAt('/series/7#volume-v1')
    await waitFor(() => expect(scroll).toHaveBeenCalledWith({ block: 'center' }))
    expect(document.activeElement).toHaveAttribute('href', '/read/v1')
  })
})
