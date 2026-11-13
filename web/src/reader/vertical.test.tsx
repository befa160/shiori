import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageInfo, VolumeDetail } from '@/api/types'
import { NO_METADATA, json, mockApi, renderAt } from '@/test/render'

// Le lecteur est une route `lazy` : chargé à froid (transformé, instrumenté pour la couverture),
// il dépassait parfois le délai d'attente du premier test sur une machine chargée.
beforeAll(() => import('@/reader/Reader'))

const VOLUME = 'b'.repeat(64)
// Bandes du webtoon réel : 800×1280 ; la 6e est cassée.
const strips: PageInfo[] = Array.from({ length: 100 }, (_, i) => ({
  id: `s${i + 1}`,
  ordinal: i,
  width: i === 5 ? null : 800,
  height: i === 5 ? null : 1280,
  media_type: 'image/jpeg',
  status: i === 5 ? 'broken' : 'ok',
}))
const volume: VolumeDetail = {
  id: VOLUME,
  size: 1,
  format: 'cbz',
  status: 'ok',
  page_count: 100,
  scanned_at: '',
  locations: [{ path: '/w/Leviathan/webtoon.cbz', library_id: 1, series_id: null }],
  issues: [],
  settings: { reading_mode: 'vertical', page_layout: 'auto', page_turn: 'reading', reading_mode_override: null, page_layout_override: null, page_turn_override: null },
  progress: null,
}

let scrollTo: ReturnType<typeof vi.fn>
beforeEach(() => {
  mockApi({
    [`/api/volumes/${VOLUME}`]: () => json(volume),
    [`/api/volumes/${VOLUME}/pages`]: () => json(strips),
    [`/api/volumes/${VOLUME}/progress`]: () => json({}),
  })
  scrollTo = vi.fn()
  vi.stubGlobal('scrollTo', scrollTo)
  scrollWindow(0)
})

/** Fait défiler la fenêtre de jsdom (qui ne défile pas d'elle-même) et prévient la page. */
function scrollWindow(y: number) {
  Object.defineProperty(window, 'scrollY', { configurable: true, value: y })
  window.dispatchEvent(new Event('scroll'))
}
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const images = () => screen.getAllByRole('img').map((i) => i.getAttribute('aria-label') ?? i.getAttribute('alt'))

describe('mode webtoon', () => {
  it('ne met dans la page que les bandes proches de l’écran, à leur place réservée, en originale', async () => {
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('img', { name: 'Page 1 sur 100' })).toBeInTheDocument()
    // jsdom : 1024×768 → colonne de 800 px, bandes de 1 280 px de haut ; une à l'écran, 3 de marge.
    expect(images()).toEqual(['Page 1 sur 100', 'Page 2 sur 100', 'Page 3 sur 100', 'Page 4 sur 100'])
    // 99 bandes de 1 280 px et la bande cassée, à la forme d'une page portrait (800 × 1,5).
    expect(screen.getByRole('main')).toHaveStyle({ width: '800px', height: `${99 * 1280 + 1200}px` })
    // 800 px affichés pour une bande de 800 px : l'originale, aucun encodage côté serveur.
    expect(screen.getByRole('img', { name: 'Page 1 sur 100' })).toHaveAttribute('src', '/api/pages/s1/image')
  })

  it('suit la bande lue dans l’adresse, et y revient à l’ouverture', async () => {
    const { router } = renderAt(`/read/${VOLUME}?page=s50`)
    await screen.findByRole('main')
    // La 6e bande, cassée, fait 1 200 px (forme d'une page portrait).
    await waitFor(() => expect(scrollTo).toHaveBeenCalledWith({ top: 48 * 1280 + 1200 }))
    // Défilement jusqu'à la 11e bande : l'adresse suit (sans réécrire l'historique à chaque pixel).
    await act(async () => scrollWindow(10 * 1280 + 5))
    // Pendant le défilement, l'annonce ne suit pas encore : elle attend l'arrêt.
    expect(screen.getByText('Page 50 sur 100', { selector: '[aria-live]' })).toBeInTheDocument()
    await waitFor(() => expect(router.state.location.search).toBe('?page=s11'))
    // L'annonce suit l'adresse au rendu suivant.
    expect(await screen.findByText('Page 11 sur 100', { selector: '[aria-live]' })).toBeInTheDocument()
    // La bande 6 n'est plus dans la page ; en y revenant, sa carte d'erreur est à sa place.
    expect(screen.queryByRole('img', { name: /^Page 6 sur 100/ })).toBeNull()
    await act(async () => scrollWindow(5 * 1280))
    expect(screen.getByRole('img', { name: /^Page 6 sur 100 :/ })).toHaveTextContent("Page illisible dans l'archive")
  })

  it('garde la bande lue quand l’écran tourne (la colonne change de largeur)', async () => {
    renderAt(`/read/${VOLUME}?page=s21`)
    await screen.findByRole('main')
    // Colonne de 800 px : la bande 21 commence à 19 × 1 280 + 1 200.
    await waitFor(() => expect(scrollTo).toHaveBeenLastCalledWith({ top: 19 * 1280 + 1200 }))
    // Écran de 400 px : colonne de 400, bandes de 640 px ; on revient à la bande 21.
    await act(async () => {
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: 400 })
      window.dispatchEvent(new Event('resize'))
    })
    await waitFor(() => expect(scrollTo).toHaveBeenLastCalledWith({ top: 19 * 640 + 600 }))
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 })
  })

  it('un clic montre les commandes ; le curseur saute à la bande choisie', async () => {
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 100' })
    const toolbar = screen.getByRole('toolbar', { hidden: true })
    expect(toolbar).toHaveClass('invisible')
    fireEvent.click(screen.getByRole('main'))
    expect(toolbar).not.toHaveClass('invisible')
    expect(within(toolbar).getByRole('link', { name: 'Retour' })).toHaveAttribute('href', '/')
    const slider = within(toolbar).getByRole('slider', { name: 'Page' })
    slider.focus()
    await userEvent.keyboard('{End}')
    await waitFor(() => expect(scrollTo).toHaveBeenCalledWith({ top: 98 * 1280 + 1200 }))
    await userEvent.keyboard('{Escape}')
    expect(toolbar).toHaveClass('invisible')
  })

  it('fait de la bande en haut de l’écran la couverture de la série', async () => {
    const inSeries = { ...volume, locations: [{ ...volume.locations[0], series_id: 7 }] }
    const series = { id: 7, library_id: 1, name: 'Leviathan', folder: 'Leviathan', publication_status: null, metadata: NO_METADATA, locked: [], settings: volume.settings, resume_volume_id: null, page_count: 100, pages_read: 0, volumes: [] }
    const fetch = mockApi({
      [`/api/volumes/${VOLUME}`]: () => json(inSeries),
      [`/api/volumes/${VOLUME}/pages`]: () => json(strips),
      [`/api/volumes/${VOLUME}/progress`]: () => json({}),
      '/api/series/7': () => json(series),
    })
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 100' })
    // La 3e bande en haut de l'écran.
    act(() => scrollWindow(2 * 1280 + 10))
    fireEvent.click(screen.getByRole('main'))
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('button', { name: 'Utiliser comme couverture' }))
    await waitFor(() => {
      const patch = fetch.mock.calls.find(([url, init]) => url === '/api/series/7' && init?.method === 'PATCH')
      expect(JSON.parse(String(patch?.[1]?.body))).toEqual({ cover_page_id: 's3' })
    })
  })

  it('reprend à la bande lue, et marque le tome terminé quand le bas de la dernière bande est à l’écran', async () => {
    const progress = { page_id: 's50', ordinal: 49, page_count: 100, finished: false, updated_at: '' }
    mockApi({
      [`/api/volumes/${VOLUME}`]: () => json({ ...volume, progress }),
      [`/api/volumes/${VOLUME}/pages`]: () => json(strips),
      [`/api/volumes/${VOLUME}/progress`]: () => json(progress),
    })
    const spy = vi.mocked(globalThis.fetch)
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('main')
    await waitFor(() => expect(scrollTo).toHaveBeenCalledWith({ top: 48 * 1280 + 1200 }))
    // Tout en bas : la première bande visible est la 100e moins une, mais le tome est fini.
    const bottom = 99 * 1280 + 1200 - 768
    await act(async () => scrollWindow(bottom))
    await waitFor(
      () => expect(spy.mock.calls.filter(([, init]) => init?.method === 'PUT').at(-1)?.[1]?.body).toBe('{"page_id":"s100"}'),
      { timeout: 2000 },
    )
  })

  it('en fin de tome, un tome sans série propose le retour', async () => {
    renderAt(`/read/${VOLUME}`)
    const end = await screen.findByRole('navigation', { name: 'Fin du tome' })
    expect(within(end).getByRole('link', { name: 'Retour' })).toHaveAttribute('href', '/')
  })
})
