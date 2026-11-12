import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PageInfo, SeriesDetail, VolumeDetail } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

const VOLUME = 'a'.repeat(64)
type Mode = 'rtl' | 'ltr' | 'vertical'
type Layout = 'single' | 'double' | 'auto'
// Page simple par défaut : jsdom simule un écran paysage, où `auto` passerait en double.
const settings = (reading_mode: Mode, page_layout: Layout = 'single') =>
  ({ reading_mode, page_layout, reading_mode_override: null, page_layout_override: null }) as const
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
    await userEvent.keyboard('{Tab}')
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
    await userEvent.keyboard('{Tab}')
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
    await userEvent.keyboard('{Tab}')
    await userEvent.click(screen.getByRole('button', { name: 'Affichage' }))
    await screen.findByRole('radio', { name: 'Simple' })
    // Côté « page suivante » : le toucher ferme seulement le panneau (Radix, à son rythme).
    fireEvent.pointerDown(main(), { pointerType: 'touch', pointerId: 9, clientX: 850, clientY: 300 })
    fireEvent.pointerUp(main(), { pointerType: 'touch', pointerId: 9, clientX: 850, clientY: 300 })
    expect(img()).toHaveAccessibleName('Page 1 sur 3')
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
