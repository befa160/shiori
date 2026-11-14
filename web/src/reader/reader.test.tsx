import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageInfo, SeriesDetail, VolumeDetail } from '@/api/types'
import { json, mockApi, renderAt, NO_METADATA } from '@/test/render'

// Le lecteur est une route `lazy` : chargé à froid (transformé, instrumenté pour la couverture),
// il dépassait parfois le délai d'attente du premier test sur une machine chargée.
beforeAll(() => import('@/reader/Reader'))

const VOLUME = 'a'.repeat(64)
type Mode = 'rtl' | 'ltr' | 'vertical'
type Layout = 'single' | 'double' | 'auto'
// Page simple par défaut : jsdom simule un écran paysage, où `auto` passerait en double.
type Turn = 'reading' | 'ltr' | 'rtl'
const settings = (reading_mode: Mode, page_layout: Layout = 'single', page_turn: Turn = 'reading') =>
  ({ reading_mode, page_layout, page_turn, reading_mode_override: null, page_layout_override: null, page_turn_override: null }) as const
const page = (n: number, status: 'ok' | 'broken' = 'ok', wide = false): PageInfo => ({
  id: `p${n}`,
  ordinal: n - 1,
  width: wide ? 2400 : 1200,
  height: 1800,
  media_type: 'image/jpeg',
  status,
})
const volume = (mode: Mode = 'rtl', layout: Layout = 'single'): VolumeDetail => ({
  id: VOLUME,
  size: 1,
  format: 'cbz',
  status: 'ok',
  page_count: 3,
  scanned_at: '',
  locations: [{ path: '/m/Toriko/Toriko T16.cbz', library_id: 1, series_id: 7 }],
  issues: [],
  settings: settings(mode, layout),
  progress: null,
})
const series: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  publication_status: null,
  metadata: NO_METADATA,
  locked: [],
  sources: [],
  resume_volume_id: null,
  page_count: 3,
  pages_read: 0,
  settings: settings('rtl'),
  volumes: [
    { volume_id: VOLUME, kind: 'volume', number: 16, number_end: null, status: 'ok', page_count: 3, cover_page_id: null, file_name: 'Toriko T16.cbz' },
  ],
}

function api(mode: Mode = 'rtl', pages = [page(1), page(2, 'broken'), page(3)], layout: Layout = 'single') {
  return mockApi({
    [`/api/volumes/${VOLUME}`]: () => json(volume(mode, layout)),
    [`/api/volumes/${VOLUME}/pages`]: () => json(pages),
    '/api/series/7': () => json(series),
  })
}

const img = () => screen.getByRole('img', { name: /^Page \d sur 3$/ })

/** Tab montre les commandes et met le focus sur « Retour » à l'image suivante : on l'attend, sinon
 * un clic juste après (machine lente) ouvrirait un panneau que ce focus tardif refermerait. */
async function showControls() {
  await userEvent.keyboard('{Tab}')
  await waitFor(() => expect(screen.getByRole('link', { name: 'Retour' })).toHaveFocus())
}
const main = () => screen.getByRole('main')

beforeEach(() => {
  // jsdom ne mesure rien : écran de 900 px de large pour les zones de tap.
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(900)
})
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('lecteur', () => {
  it('reste sombre, quel que soit le thème de l’interface', async () => {
    document.documentElement.dataset.theme = 'light'
    api('rtl')
    renderAt(`/read/${VOLUME}`)
    // Premier test du fichier : le lecteur (route `lazy`) peut encore se charger à froid.
    await screen.findByRole('img', { name: 'Page 1 sur 3' }, { timeout: 5000 })
    expect(document.documentElement.dataset.theme).toBe('dark')
    expect(document.documentElement).toHaveClass('dark')
  })

  it('affiche la page 1 à la taille de l’écran, puis avance au clavier dans le sens manga', async () => {
    api('rtl')
    const { router } = renderAt(`/read/${VOLUME}`)
    // jsdom : écran 1024×768, page 1200×1800 → 512 px affichés → largeur fixe 640 (< 1200).
    expect(await screen.findByRole('img', { name: 'Page 1 sur 3' })).toHaveAttribute('src', '/api/pages/p1/image?w=640&f=avif')
    // Le titre suit, une fois la série chargée (elle dépend du tome).
    await waitFor(() => expect(document.title).toBe('Toriko · Tome 16 · Shiori'))
    await userEvent.keyboard('{ArrowRight}')
    expect(img()).toHaveAccessibleName('Page 1 sur 3') // en manga, la flèche droite recule
    await userEvent.keyboard('{ArrowLeft}')
    // Page 2 cassée : une erreur explicite à sa place, jamais une autre image.
    const broken = screen.getByRole('img', { name: /^Page 2 sur 3 :/ })
    expect(broken).toHaveTextContent("Page illisible dans l'archive")
    expect(router.state.location.search).toBe('?page=p2')
    await userEvent.keyboard('{End}')
    expect(img()).toHaveAccessibleName('Page 3 sur 3')
    expect(screen.getByText('Page 3 sur 3', { selector: '[aria-live]' })).toBeInTheDocument()
  })

  it('tourne les pages au tap selon le sens de lecture et affiche les commandes au centre', async () => {
    api('ltr')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    const tap = (x: number) => {
      fireEvent.pointerDown(main(), { clientX: x, clientY: 300 })
      fireEvent.pointerUp(main(), { clientX: x, clientY: 300 })
    }
    tap(850)
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    tap(50)
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
    const toolbar = screen.getByRole('toolbar', { hidden: true })
    expect(toolbar).toHaveClass('invisible')
    tap(450)
    // Au centre, le tap attend un éventuel second tap (zoom) avant de montrer les commandes.
    expect(toolbar).toHaveClass('invisible')
    await waitFor(() => expect(toolbar).not.toHaveClass('invisible'))
    expect(within(toolbar).getByRole('link', { name: 'Retour' })).toHaveAttribute('href', '/series/7')
    await userEvent.click(within(toolbar).getByRole('button', { name: 'Page suivante' }))
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    await userEvent.keyboard('{Escape}')
    expect(toolbar).toHaveClass('invisible')
    await userEvent.keyboard('{Escape}')
    expect(toolbar).toHaveClass('invisible') // Échap ne fait que masquer
    // Au clavier, Tab montre les commandes et y place le focus.
    await userEvent.keyboard('{Tab}')
    expect(toolbar).not.toHaveClass('invisible')
    await waitFor(() => expect(within(toolbar).getByRole('link', { name: 'Retour' })).toHaveFocus())
  })

  it('tourne les pages au balayage, sauf depuis le bord de l’écran', async () => {
    api('rtl')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    const swipe = (from: number, to: number) => {
      fireEvent.pointerDown(main(), { clientX: from, clientY: 300 })
      fireEvent.pointerUp(main(), { clientX: to, clientY: 300 })
    }
    swipe(10, 300) // part du bord : geste système
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
    swipe(300, 300 + 120 * -1) // vers la gauche en manga : recule (déjà au début)
    swipe(300, 420) // vers la droite en manga : avance
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
  })

  it('ne tourne jamais la page pendant un pincement à deux doigts, ni au clic droit', async () => {
    api('ltr')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    const touch = { pointerType: 'touch' }
    fireEvent.pointerDown(main(), { ...touch, pointerId: 1, clientX: 300, clientY: 300 })
    fireEvent.pointerDown(main(), { ...touch, pointerId: 2, clientX: 600, clientY: 300 })
    fireEvent.pointerUp(main(), { ...touch, pointerId: 2, clientX: 750, clientY: 300 })
    fireEvent.pointerUp(main(), { ...touch, pointerId: 1, clientX: 150, clientY: 300 })
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
    // Après le pincement, un tap redevient un tap.
    fireEvent.pointerDown(main(), { ...touch, pointerId: 3, clientX: 850, clientY: 300 })
    fireEvent.pointerUp(main(), { ...touch, pointerId: 3, clientX: 850, clientY: 300 })
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    // Clic droit sur le côté « page précédente » : rien.
    const mouse = { pointerType: 'mouse', button: 2, pointerId: 4 }
    fireEvent.pointerDown(main(), { ...mouse, clientX: 50, clientY: 300 })
    fireEvent.pointerUp(main(), { ...mouse, clientX: 50, clientY: 300 })
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    // Un geste interrompu par le système ne laisse rien en suspens.
    fireEvent.pointerDown(main(), { ...touch, pointerId: 5, clientX: 850, clientY: 300 })
    fireEvent.pointerCancel(main(), { ...touch, pointerId: 5 })
    fireEvent.pointerUp(main(), { ...touch, pointerId: 5, clientX: 850, clientY: 300 })
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
  })

  it('au clavier dans les commandes, les touches vont au bouton ou au curseur qui a le focus', async () => {
    api('ltr')
    const { router } = renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    await userEvent.keyboard('{Tab}')
    const back = screen.getByRole('link', { name: 'Retour' })
    await waitFor(() => expect(back).toHaveFocus())
    // Espace sur « Retour » ne tourne pas la page.
    await userEvent.keyboard(' ')
    expect(router.state.location.search).toBe('')
    // Le curseur : chaque appui de flèche change la page (validation immédiate au clavier).
    const slider = screen.getByRole('slider', { name: 'Page' })
    slider.focus()
    await userEvent.keyboard('{ArrowRight}')
    expect(router.state.location.search).toBe('?page=p2')
    expect(screen.getByText('2 / 3')).toBeInTheDocument()
  })

  it('reprend la page donnée dans l’adresse', async () => {
    api('ltr')
    renderAt(`/read/${VOLUME}?page=p3`)
    expect(await screen.findByRole('img', { name: 'Page 3 sur 3' })).toBeInTheDocument()
    await userEvent.keyboard('{ArrowLeft}') // gauche à droite : la flèche gauche recule
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
  })

  it('retente en originale si le rendu échoue, puis montre la cause donnée par le serveur', async () => {
    const fetch = api('ltr')
    renderAt(`/read/${VOLUME}`)
    const first = await screen.findByRole('img', { name: 'Page 1 sur 3' })
    fireEvent.error(first)
    expect(img()).toHaveAttribute('src', '/api/pages/p1/image')
    fetch.mockImplementation(async () => json({ code: 'volume_changed', message: 'le fichier a changé' }, 409))
    await act(async () => fireEvent.error(img()))
    expect(await screen.findByRole('img', { name: /^Page 1 sur 3 :/ })).toHaveTextContent(
      'Le fichier a changé depuis le scan : à rescanner',
    )
  })

  it('précharge les pages voisines', async () => {
    const loaded: string[] = []
    vi.stubGlobal(
      'Image',
      class {
        set src(url: string) {
          loaded.push(url)
        }
      },
    )
    api('ltr')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    // La page 2 est cassée : rien à précharger pour elle.
    expect(loaded).toEqual(['/api/pages/p3/image?w=640&f=avif'])
  })

  it('parle la langue enregistrée, même hors du cadre commun', async () => {
    api('ltr')
    mockApi({
      '/api/settings': () => json({ language: 'en' }),
      [`/api/volumes/${VOLUME}`]: () => json(volume('ltr')),
      [`/api/volumes/${VOLUME}/pages`]: () => json([page(1)]),
      '/api/series/7': () => json(series),
    })
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('img', { name: 'Page 1 of 1' })).toBeInTheDocument()
  })
})

describe('double page', () => {
  const book = [page(1), page(2), page(3), page(4, 'ok', true), page(5), page(6)]
  const shown = () => screen.getAllByRole('img').map((i) => i.getAttribute('alt'))

  it('en paysage, auto montre deux pages : couverture seule, double page seule, paires dans le sens manga', async () => {
    api('rtl', book, 'auto')
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('img', { name: 'Page 1 sur 6' })).toBeInTheDocument()
    await userEvent.keyboard('{ArrowLeft}')
    // Pages 2 et 3 côte à côte : en manga, la 2 est à droite (rangée inversée).
    expect(shown()).toEqual(['Page 2 sur 6', 'Page 3 sur 6'])
    expect(screen.getByRole('img', { name: 'Page 2 sur 6' }).parentElement).toHaveClass('flex-row-reverse')
    expect(screen.getByText('Pages 2 et 3 sur 6', { selector: '[aria-live]' })).toBeInTheDocument()
    expect(screen.getByText('2–3 / 6')).toBeInTheDocument()
    // Chaque page reçoit la moitié de l'écran : 1024×768, page 1200×1800 → 512 px → 640.
    expect(screen.getByRole('img', { name: 'Page 2 sur 6' })).toHaveAttribute('src', '/api/pages/p2/image?w=640&f=avif')
    await userEvent.keyboard('{ArrowLeft}')
    expect(shown()).toEqual(['Page 4 sur 6'])
    await userEvent.keyboard('{ArrowLeft}')
    expect(shown()).toEqual(['Page 5 sur 6', 'Page 6 sur 6'])
  })

  it('le panneau Affichage change la disposition, décale les paires et coupe les doubles pages', async () => {
    api('ltr', book, 'double')
    renderAt(`/read/${VOLUME}?page=p2`)
    expect(await screen.findByRole('img', { name: 'Page 2 sur 6' })).toBeInTheDocument()
    expect(shown()).toEqual(['Page 2 sur 6', 'Page 3 sur 6'])
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('button', { name: "Décaler d'une page" }))
    // Décalé : la page 2 est seule, la 3 démarre la paire suivante.
    expect(shown()).toEqual(['Page 2 sur 6'])
    // En simple, la double page est coupée en deux moitiés successives (gauche d'abord en BD).
    await userEvent.click(screen.getByRole('radio', { name: 'Simple' }))
    // Échap ferme d'abord le panneau, puis masque les commandes : les touches reviennent aux pages.
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('radio', { name: 'Simple' })).toBeNull()
    expect(screen.getByRole('toolbar', { hidden: true })).not.toHaveClass('invisible')
    await userEvent.keyboard('{Escape}')
    expect(screen.getByRole('toolbar', { hidden: true })).toHaveClass('invisible')
    await userEvent.keyboard('{End}')
    expect(shown()).toEqual(['Page 6 sur 6'])
    await userEvent.keyboard('{ArrowLeft}{ArrowLeft}')
    expect(shown()).toEqual(['Page 4 sur 6 (moitié droite)'])
    await userEvent.keyboard('{ArrowLeft}')
    expect(shown()).toEqual(['Page 4 sur 6 (moitié gauche)'])
    // L'annonce aux lecteurs d'écran dit aussi de quelle moitié il s'agit.
    expect(screen.getByText('Page 4 sur 6 (moitié gauche)', { selector: '[aria-live]' })).toBeInTheDocument()
    // Et on repart en avant à travers les deux moitiés, sans rester bloqué sur la première.
    await userEvent.keyboard('{ArrowRight}')
    expect(shown()).toEqual(['Page 4 sur 6 (moitié droite)'])
    await userEvent.keyboard('{ArrowRight}')
    expect(shown()).toEqual(['Page 5 sur 6'])
    await userEvent.keyboard('{ArrowLeft}{ArrowLeft}')
    // L'image entière, deux fois la largeur de la boîte, demandée pour deux écrans de large.
    const halfImage = screen.getByRole('img', { name: 'Page 4 sur 6 (moitié gauche)' })
    expect(halfImage).toHaveStyle({ width: '200%' })
    // Doubles pages entières : une seule vue.
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('radio', { name: 'Entières' }))
    expect(shown()).toEqual(['Page 4 sur 6'])
  })
})

describe('panneau Affichage ouvert', () => {
  it('toucher la page pour le fermer ne la tourne pas', async () => {
    api('ltr', [page(1), page(2), page(3)], 'single')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await screen.findByRole('radio', { name: 'Simple' })
    // Côté « page suivante » : le toucher ferme seulement le panneau (Radix, à son rythme).
    fireEvent.pointerDown(main(), { pointerType: 'touch', pointerId: 9, clientX: 850, clientY: 300 })
    fireEvent.pointerUp(main(), { pointerType: 'touch', pointerId: 9, clientX: 850, clientY: 300 })
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
  })
})

describe('progression, réglages de la série, tome suivant', () => {
  const NEXT = 'c'.repeat(64)
  const progress = (ordinal: number) => ({ page_id: `p${ordinal + 1}`, ordinal, page_count: 3, finished: ordinal === 2, updated_at: '' })
  const twoVolumes: SeriesDetail = {
    ...series,
    volumes: [...series.volumes, { ...series.volumes[0], volume_id: NEXT, number: 17, file_name: 'Toriko T17.cbz' }],
  }
  function serve(over: Partial<VolumeDetail> = {}, extra: Record<string, () => Response> = {}) {
    return mockApi({
      [`/api/volumes/${VOLUME}`]: () => json({ ...volume('ltr'), ...over }),
      [`/api/volumes/${VOLUME}/pages`]: () => json([page(1), page(2, 'broken'), page(3)]),
      '/api/series/7': () => json(twoVolumes),
      ...extra,
    })
  }
  /** Corps des requêtes envoyées avec cette méthode. */
  const sent = (fetch: ReturnType<typeof mockApi>, method: string) =>
    fetch.mock.calls
      .filter(([, init]) => init?.method === method)
      .map(([url, init]) => [url, JSON.parse(init?.body as string)])

  it('reprend à la dernière page lue ; un tome terminé repart du début', async () => {
    serve({ progress: progress(1) })
    const { unmount } = renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    unmount()
    serve({ progress: progress(2) })
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('img', { name: 'Page 1 sur 3' })).toBeInTheDocument()
  })

  it("redemande l'accueil au retour : « Continuer la lecture » et les séries suivent la lecture", async () => {
    const fetch = serve({}, {
      [`/api/volumes/${VOLUME}/progress`]: () => json(progress(1)),
      '/api/libraries': () => json([]),
      '/api/series': () => json([]),
      '/api/continue': () => json([]),
    })
    const calls = (url: string) => fetch.mock.calls.filter(([u, init]) => u === url && !init?.method).length
    // Données gardées 30 s comme en production : seule la péremption après l'enregistrement
    // les fait redemander.
    const { router } = renderAt('/', { staleTime: 30_000 })
    await waitFor(() => expect(calls('/api/continue')).toBe(1))
    await act(() => router.navigate(`/read/${VOLUME}`))
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    await userEvent.keyboard('{ArrowRight}')
    await waitFor(() => expect(sent(fetch, 'PUT')).toHaveLength(1), { timeout: 2000 })
    await act(() => router.navigate('/'))
    await waitFor(() => expect(calls('/api/continue')).toBe(2))
    expect(calls('/api/series')).toBe(2)
  })

  it('enregistre la page lue une seconde après, aussitôt en arrière-plan ou en quittant', async () => {
    const url = `/api/volumes/${VOLUME}/progress`
    const fetch = serve({}, { [url]: () => json(progress(0)) })
    const { unmount } = renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    // Feuilleter n'envoie que la page où l'on s'arrête.
    await userEvent.keyboard('{ArrowRight}{ArrowRight}')
    expect(sent(fetch, 'PUT')).toEqual([])
    await waitFor(() => expect(sent(fetch, 'PUT')).toEqual([[url, { page_id: 'p3' }]]), { timeout: 2000 })
    expect(fetch.mock.calls.find(([, init]) => init?.method === 'PUT')?.[1]).toMatchObject({ keepalive: true })
    // Onglet en arrière-plan : tout de suite, sans attendre la seconde.
    await userEvent.keyboard('{ArrowLeft}')
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
    act(() => void document.dispatchEvent(new Event('visibilitychange')))
    Reflect.deleteProperty(document, 'visibilityState')
    expect(sent(fetch, 'PUT').at(-1)).toEqual([url, { page_id: 'p2' }])
    // Quitter le lecteur aussi.
    await userEvent.keyboard('{ArrowLeft}')
    unmount()
    expect(sent(fetch, 'PUT').at(-1)).toEqual([url, { page_id: 'p1' }])
    expect(sent(fetch, 'PUT')).toHaveLength(3)
  })

  it('en manga, tourne les pages de gauche à droite si le réglage le demande, sans changer l’ordre des pages', async () => {
    const fetch = serve(
      { settings: settings('rtl', 'double', 'ltr') },
      {
        [`/api/volumes/${VOLUME}/pages`]: () => json([page(1), page(2), page(3)]),
        '/api/series/7/settings': () => json(settings('rtl', 'double', 'rtl')),
      },
    )
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    // Flèche droite et tap à droite avancent ; la double page reste lue de droite à gauche.
    await userEvent.keyboard('{ArrowRight}')
    const pair = await screen.findAllByRole('img', { name: /^Page [23] sur 3/ })
    // Paire affichée de droite à gauche (page 2 à droite) : sens de lecture inchangé.
    expect(pair).toHaveLength(2)
    expect(pair[0].closest('.flex-row-reverse')).not.toBeNull()
    fireEvent.pointerDown(main(), { clientX: 50, clientY: 300 })
    fireEvent.pointerUp(main(), { clientX: 50, clientY: 300 })
    expect(await screen.findByRole('img', { name: 'Page 1 sur 3' })).toBeInTheDocument()
    fireEvent.pointerDown(main(), { clientX: 850, clientY: 300 })
    fireEvent.pointerUp(main(), { clientX: 850, clientY: 300 })
    expect(await screen.findAllByRole('img', { name: /^Page [23] sur 3/ })).toHaveLength(2)
    // Le curseur et les boutons suivent aussi : « Page suivante » à droite, curseur de gauche à droite.
    await showControls()
    const toolbar = screen.getByRole('toolbar')
    const buttons = within(toolbar).getAllByRole('button', { name: /^Page (suivante|précédente)$/ })
    expect(buttons.map((b) => b.getAttribute('aria-label'))).toEqual(['Page précédente', 'Page suivante'])
    expect(within(toolbar).getByRole('slider').closest('[dir]')).toHaveAttribute('dir', 'ltr')
    // Le panneau Affichage le change pour la série.
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    expect(await screen.findByRole('radio', { name: 'Toucher à droite avance' })).toHaveAttribute('aria-checked', 'true')
    await userEvent.click(screen.getByRole('radio', { name: 'Toucher à gauche avance' }))
    expect(sent(fetch, 'PATCH')).toEqual([['/api/series/7/settings', { page_turn: 'rtl' }]])
  })

  it('le sens de lecture et la disposition choisis sont retenus pour la série', async () => {
    vi.stubGlobal('scrollTo', vi.fn())
    const fetch = serve({ settings: settings('rtl') }, { '/api/series/7/settings': () => json(settings('ltr')) })
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    expect(await screen.findByRole('radio', { name: 'Manga' })).toHaveAttribute('aria-checked', 'true')
    await userEvent.click(screen.getByRole('radio', { name: 'BD' }))
    await userEvent.click(screen.getByRole('radio', { name: 'Double' }))
    expect(sent(fetch, 'PATCH')).toEqual([
      ['/api/series/7/settings', { reading_mode: 'ltr' }],
      ['/api/series/7/settings', { page_layout: 'double' }],
    ])
    // Appliqués tout de suite : en BD, la flèche droite avance.
    await userEvent.keyboard('{Escape}{Escape}{ArrowRight}')
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    // Webtoon : le lecteur passe en défilement vertical, et le panneau y reste pour en revenir.
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('radio', { name: 'Webtoon' }))
    expect(await screen.findByRole('navigation', { name: 'Fin du tome' })).toBeInTheDocument()
    expect(sent(fetch, 'PATCH').at(-1)).toEqual(['/api/series/7/settings', { reading_mode: 'vertical' }])
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    expect(await screen.findByRole('radio', { name: 'Webtoon' })).toHaveAttribute('aria-checked', 'true')
  })

  it('un échec d’enregistrement est signalé, sans bloquer la lecture', async () => {
    // Aucune réponse prévue pour la progression ni les réglages : le serveur répond 404.
    const fetch = serve()
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    expect(await screen.findByText("La page lue n'a pas pu être enregistrée", {}, { timeout: 2000 })).toBeInTheDocument()
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('radio', { name: 'Manga' }))
    expect(await screen.findByText("Le réglage n'a pas pu être enregistré pour la série")).toBeInTheDocument()
    // Le réglage reste appliqué pour la séance, même quand le tome est redemandé au serveur
    // (retour sur l'onglet) : en manga, la flèche gauche avance.
    const asked = fetch.mock.calls.filter(([url]) => url === `/api/volumes/${VOLUME}`).length
    await act(async () => void window.dispatchEvent(new Event('visibilitychange')))
    await waitFor(() => expect(fetch.mock.calls.filter(([url]) => url === `/api/volumes/${VOLUME}`).length).toBeGreaterThan(asked))
    await userEvent.keyboard('{Escape}{Escape}{ArrowLeft}')
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
  })

  it('fait de la page affichée la couverture de la série, et le dit', async () => {
    const fetch = serve({}, { '/api/series/7': (init?: RequestInit) => json(init?.method === 'PATCH' ? { ...twoVolumes, metadata: { ...NO_METADATA, cover_page_id: 'p1' } } : twoVolumes) })
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    // Plein écran, sans barre d'onglets : les notifications ne s'en écartent pas.
    expect(document.querySelector('[data-reader]')).not.toBeNull()
    await showControls()
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await userEvent.click(await screen.findByRole('button', { name: 'Utiliser comme couverture' }))
    expect(await screen.findByText('Couverture de la série changée')).toBeInTheDocument()
    const patch = fetch.mock.calls.find(([url, init]) => url === '/api/series/7' && init?.method === 'PATCH')
    expect(JSON.parse(String(patch?.[1]?.body))).toEqual({ cover_page_id: 'p1' })
  })

  it('après la dernière page, les commandes proposent le tome suivant', async () => {
    serve()
    const { router } = renderAt(`/read/${VOLUME}?page=p3`)
    await screen.findByRole('img', { name: 'Page 3 sur 3' })
    const toolbar = screen.getByRole('toolbar', { hidden: true })
    // Dès la dernière page (une fois la série connue), le bouton est dans la barre.
    expect(await within(toolbar).findByRole('link', { name: 'Tome suivant' })).toHaveAttribute('href', `/read/${NEXT}`)
    expect(toolbar).toHaveClass('invisible')
    await userEvent.keyboard('{ArrowRight}')
    expect(toolbar).not.toHaveClass('invisible')
    await userEvent.click(within(toolbar).getByRole('link', { name: 'Tome suivant' }))
    expect(router.state.location.pathname).toBe(`/read/${NEXT}`)
  })
})

describe('zoom au double tap', () => {
  it('double, puis simple : le point touché reste visible, tourner la page ou un double tap le quitte', async () => {
    api('ltr')
    renderAt(`/read/${VOLUME}`)
    await screen.findByRole('img', { name: 'Page 1 sur 3' })
    const tap = (x: number) => {
      fireEvent.pointerDown(main(), { clientX: x, clientY: 300 })
      fireEvent.pointerUp(main(), { clientX: x, clientY: 300 })
    }
    const wait = () => act(() => new Promise((done) => setTimeout(done, 350)))
    tap(450)
    tap(460)
    // Deux fois la place de l'écran (2048×1536) : page de 1 024 px, demandée en 1 080.
    expect(img()).toHaveAttribute('src', '/api/pages/p1/image?w=1080&f=avif')
    expect(img()).toHaveStyle({ width: '1024px' })
    expect(main()).toHaveClass('overflow-auto')
    // On s'y déplace et on y pince comme d'habitude ; seul notre double tap zoome.
    expect(main()).toHaveStyle({ touchAction: 'manipulation' })
    // Le premier tap n'a pas montré les commandes.
    await wait()
    expect(screen.getByRole('toolbar', { hidden: true })).toHaveClass('invisible')
    // Zoomé, un tap sur le côté ne tourne pas la page ; un double tap revient à l'écran.
    tap(850)
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
    tap(850)
    expect(main()).toHaveClass('overflow-hidden')
    await wait()
    tap(850)
    expect(screen.getByRole('img', { name: /^Page 2 sur 3/ })).toBeInTheDocument()
    // Tourner la page quitte le zoom.
    tap(450)
    tap(450)
    expect(main()).toHaveClass('overflow-auto')
    await userEvent.keyboard('{ArrowRight}')
    expect(img()).toHaveAccessibleName('Page 3 sur 3')
    expect(main()).toHaveClass('overflow-hidden')
  })
})

describe('tome absent ou vide', () => {
  it('identifiant invalide : page introuvable', async () => {
    mockApi({})
    renderAt('/read/pas-un-hash')
    expect(await screen.findByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
  })

  it('tome inconnu : erreur explicite', async () => {
    mockApi({})
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByRole('alert')).toHaveTextContent('Tome introuvable')
  })

  it('tome sans page : le dit et propose de revenir', async () => {
    api('ltr', [])
    renderAt(`/read/${VOLUME}`)
    expect(await screen.findByText("Ce tome n'a aucune page lisible.")).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Retour' })).toHaveAttribute('href', '/series/7')
  })
})
