import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageInfo, SeriesDetail, VolumeDetail } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

const VOLUME = 'a'.repeat(64)
const settings = (reading_mode: 'rtl' | 'ltr' | 'vertical') =>
  ({ reading_mode, page_layout: 'auto', reading_mode_override: null, page_layout_override: null }) as const
const page = (n: number, status: 'ok' | 'broken' = 'ok'): PageInfo => ({
  id: `p${n}`,
  ordinal: n - 1,
  width: 1200,
  height: 1800,
  media_type: 'image/jpeg',
  status,
})
const volume = (mode: 'rtl' | 'ltr' | 'vertical' = 'rtl'): VolumeDetail => ({
  id: VOLUME,
  size: 1,
  format: 'cbz',
  status: 'ok',
  page_count: 3,
  scanned_at: '',
  locations: [{ path: '/m/Toriko/Toriko T16.cbz', library_id: 1, series_id: 7 }],
  issues: [],
  settings: settings(mode),
  progress: null,
})
const series: SeriesDetail = {
  id: 7,
  library_id: 1,
  name: 'Toriko',
  folder: 'Toriko',
  settings: settings('rtl'),
  volumes: [
    { volume_id: VOLUME, kind: 'volume', number: 16, number_end: null, status: 'ok', page_count: 3, cover_page_id: null, file_name: 'Toriko T16.cbz' },
  ],
}

function api(mode: 'rtl' | 'ltr' | 'vertical' = 'rtl', pages = [page(1), page(2, 'broken'), page(3)]) {
  return mockApi({
    [`/api/volumes/${VOLUME}`]: () => json(volume(mode)),
    [`/api/volumes/${VOLUME}/pages`]: () => json(pages),
    '/api/series/7': () => json(series),
  })
}

const img = () => screen.getByRole('img', { name: /^Page \d sur 3$/ })
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
    expect(toolbar).not.toHaveClass('invisible')
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

  it('reprend la page donnée dans l’adresse et lit le webtoon en paginé en attendant le lot 3.7', async () => {
    api('vertical')
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

describe('tome absent ou vide', () => {
  it('identifiant invalide : page introuvable', () => {
    mockApi({})
    renderAt('/read/pas-un-hash')
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
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
