import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SeriesDetail } from '@/api/types'
import { fakeCaches } from '@/test/caches'
import { ADMIN, NO_METADATA, json, me, renderAt } from '@/test/render'
import { resetForTests } from './downloads'
import { queueProgress } from './progress'

const settings = { reading_mode: 'rtl', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null } as const
const volume = (id: string, number: number) =>
  ({ volume_id: id, kind: 'volume', number, number_end: null, status: 'ok', page_count: 2, cover_page_id: null, file_name: `Toriko T0${number}.cbz` }) as const
const detail: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  publication_status: null,
  metadata: NO_METADATA,
  locked: [],
  sources: [],
  resume_volume_id: null,
  page_count: 4,
  pages_read: 0,
  settings,
  volumes: [volume('a', 1), volume('b', 2)],
}
const pages = [{ id: 'p0', ordinal: 0, width: 100, height: 100, media_type: 'image/jpeg', status: 'broken' }, { id: 'p1', ordinal: 0, width: 1200, height: 1800, media_type: 'image/jpeg', status: 'ok' }]

/** Le serveur ; `hold` retient les pages pour voir un téléchargement en cours. */
function serve({ hold = false } = {}) {
  let release = () => {}
  const held = new Promise<void>((resolve) => (release = resolve))
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      if (url.startsWith('/api/pages/')) {
        // Comme un vrai `fetch`, une annulation arrête la requête retenue.
        const aborted = new Promise<never>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('arrêt', 'AbortError'))))
        if (hold) await Promise.race([held, aborted])
        return new Response(new Uint8Array(2_000_000))
      }
      // Page lue hors ligne : le serveur ne la reçoit pas encore, elle reste dans la file.
      if (url.endsWith('/progress')) throw new TypeError('Failed to fetch')
      if (url === '/api/series/7') return json(detail)
      if (url.endsWith('/pages')) return json(pages)
      if (url === '/api/settings') return json({ language: 'fr', theme: 'dark' })
      if (url === '/api/me') return json(me(ADMIN))
      if (['/api/libraries', '/api/series', '/api/continue'].includes(url)) return json([])
      if (url === '/api/admin/scans/current') return new Response(null, { status: 204 })
      return json({ id: url })
    }),
  )
  return release
}

beforeEach(() => {
  localStorage.clear()
  fakeCaches()
  vi.stubGlobal('screen', { width: 800, height: 1280 })
  resetForTests()
})
afterEach(() => vi.unstubAllGlobals())

describe('tomes téléchargés, dans l’interface', () => {
  it('se téléchargent depuis la page série et se gèrent dans « Tomes téléchargés »', async () => {
    const release = serve({ hold: true })
    vi.stubGlobal('navigator', { ...navigator, onLine: true, storage: { persist: vi.fn(), estimate: async () => ({ usage: 4e6, quota: 1e9 }) } })
    renderAt('/series/7')
    const main = within(screen.getByRole('main'))
    await userEvent.click(await main.findByRole('button', { name: 'Actions pour Tome 1' }))
    await userEvent.click(await screen.findByRole('menuitem', { name: 'Télécharger pour lire hors ligne' }))
    // En cours : pourcentage sur la carte, et le menu propose d'annuler.
    expect(await main.findByText('↓ 0 %')).toBeInTheDocument()
    await userEvent.click(main.getByRole('button', { name: 'Actions pour Tome 1' }))
    expect(await screen.findByRole('menuitem', { name: 'Annuler le téléchargement' })).toBeInTheDocument()
    await userEvent.keyboard('{Escape}')
    release()
    expect(await main.findByText('Téléchargé')).toBeInTheDocument()
    // Le reste de la série d'un coup ; le bouton disparaît quand tout est gardé.
    await userEvent.click(main.getByRole('button', { name: 'Télécharger la série' }))
    await waitFor(() => expect(main.getAllByText('Téléchargé')).toHaveLength(2))
    expect(main.queryByRole('button', { name: 'Télécharger la série' })).toBeNull()

    // La page des tomes téléchargés, reprise à la page lue hors ligne.
    localStorage.setItem('shiori.user', String(ADMIN.id))
    queueProgress('b', 'p1')
    renderAt('/downloads')
    const page = within(screen.getAllByRole('main').at(-1)!)
    expect(await page.findByText('4 Mo utilisés sur cet appareil, 996 Mo disponibles')).toBeInTheDocument()
    expect(page.getByRole('link', { name: 'Lire Toriko · Tome 2' })).toHaveAttribute('href', '/read/b?page=p1')
    expect(page.getByRole('link', { name: 'Lire Toriko · Tome 1' })).toHaveAttribute('href', '/read/a')
    expect(page.getAllByText(/1 page · 2 Mo/)).toHaveLength(2)
    await userEvent.click(page.getByRole('button', { name: 'Supprimer Toriko · Tome 1 de cet appareil' }))
    await waitFor(() => expect(page.queryByRole('link', { name: 'Lire Toriko · Tome 1' })).toBeNull())
  })

  it('montre la file d’attente et l’annule', async () => {
    serve({ hold: true })
    renderAt('/series/7')
    const main = within(screen.getByRole('main'))
    await userEvent.click(await main.findByRole('button', { name: 'Télécharger la série' }))
    renderAt('/downloads')
    const page = within(screen.getAllByRole('main').at(-1)!)
    expect(await page.findByRole('progressbar', { name: /^Tome 1 : \d pages sur \d$/ })).toBeInTheDocument()
    expect(page.getByText('En attente')).toBeInTheDocument()
    await userEvent.click(page.getByRole('button', { name: 'Annuler le téléchargement de Tome 2' }))
    await userEvent.click(page.getByRole('button', { name: 'Annuler le téléchargement de Tome 1' }))
    expect(await page.findByText(/Aucun tome téléchargé/)).toBeInTheDocument()
  })

  it('prévient quand la lecture hors ligne ne marchera pas sur cette installation', async () => {
    serve()
    Object.defineProperty(navigator, 'serviceWorker', { configurable: true, value: undefined })
    renderAt('/downloads')
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Lecture hors ligne indisponible ici')
    expect(within(alert).getByRole('link', { name: /Voir la documentation/ })).toHaveAttribute('href', 'https://befa160.github.io/shiori/guide/reading#lire-hors-ligne')
  })

  it('hors ligne, l’avis mène aux tomes téléchargés', async () => {
    serve()
    vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(false)
    renderAt('/')
    await userEvent.click(await screen.findByRole('link', { name: 'Tomes téléchargés' }))
    expect(await screen.findByRole('heading', { level: 1, name: 'Tomes téléchargés' })).toBeInTheDocument()
    vi.restoreAllMocks()
  })
})
