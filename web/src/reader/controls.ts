// Logique du lecteur, en fonctions pures : largeur demandée, touches, zones de tap, balayages.
// Le sens de lecture change le sens physique : en manga (`rtl`), « suivante » est à gauche.
import { WIDTHS, type Width, pageImageUrl } from '@/api/client'
import type { PageInfo } from '@/api/types'

export type Direction = 'rtl' | 'ltr'
export type Move = 'next' | 'previous' | 'first' | 'last'

/** Largeur affichée (px CSS) d'une page ajustée à l'écran, sans la déformer ni l'agrandir
 * au-delà de l'écran. Sans dimensions connues (page cassée) : une page portrait 2:3. */
export function fittedWidth(viewWidth: number, viewHeight: number, page: Pick<PageInfo, 'width' | 'height'>): number {
  const ratio = page.width && page.height ? page.width / page.height : 2 / 3
  return Math.min(viewWidth, viewHeight * ratio)
}

/** Largeur à demander au serveur : la plus petite de la liste fixe qui couvre la largeur affichée
 * en pixels réels ; `null` = l'originale (AVIF, que le serveur ne redimensionne pas, ou page plus
 * étroite que la largeur choisie : on n'agrandit jamais). */
export function requestWidth(displayed: number, pixelRatio: number, page: Pick<PageInfo, 'width' | 'media_type'>): Width | null {
  if (page.media_type === 'image/avif') return null
  const target = displayed * pixelRatio
  const width = WIDTHS.find((w) => w >= target) ?? WIDTHS[WIDTHS.length - 1]
  return page.width !== null && width >= page.width ? null : width
}

/** URL d'une page à la taille de l'écran (AVIF), ou l'originale. */
export function pageUrl(
  page: PageInfo,
  view: { width: number; height: number },
  original = false,
  pixelRatio = window.devicePixelRatio,
): string {
  const width = original ? null : requestWidth(fittedWidth(view.width, view.height, page), pixelRatio, page)
  return pageImageUrl(page.id, width === null ? undefined : { width, format: 'avif' })
}

/** Touche clavier → déplacement, selon le sens de lecture. */
export function keyMove(key: string, shift: boolean, direction: Direction): Move | null {
  const forward = direction === 'rtl' ? 'ArrowLeft' : 'ArrowRight'
  const backward = direction === 'rtl' ? 'ArrowRight' : 'ArrowLeft'
  switch (key) {
    case forward:
    case 'PageDown':
      return 'next'
    case backward:
    case 'PageUp':
      return 'previous'
    case ' ':
      return shift ? 'previous' : 'next'
    case 'Home':
      return 'first'
    case 'End':
      return 'last'
    default:
      return null
  }
}

/** Tap : tiers gauche / droit pour tourner la page, centre pour afficher les commandes. */
export function tapAction(x: number, width: number, direction: Direction): 'next' | 'previous' | 'toggle' {
  const third = width / 3
  if (x > third && x < width - third) return 'toggle'
  const left = x <= third
  return left === (direction === 'rtl') ? 'next' : 'previous'
}

/** Marge des bords ignorée : les gestes système (retour arrière) y commencent. */
export const EDGE = 24
const MIN_SWIPE = 50

/** Balayage horizontal net (≥ 50 px, plus horizontal que vertical), hors des bords de l'écran. */
export function swipeAction(
  start: { x: number; y: number },
  end: { x: number; y: number },
  width: number,
  direction: Direction,
): 'next' | 'previous' | null {
  if (start.x < EDGE || start.x > width - EDGE) return null
  const dx = end.x - start.x
  const dy = end.y - start.y
  if (Math.abs(dx) < MIN_SWIPE || Math.abs(dx) < Math.abs(dy) * 1.5) return null
  // En manga, la page se tourne vers la droite : balayer vers la droite avance.
  return dx > 0 === (direction === 'rtl') ? 'next' : 'previous'
}

/** Index après un déplacement, borné au tome. */
export function moveIndex(index: number, move: Move, count: number): number {
  const last = Math.max(0, count - 1)
  switch (move) {
    case 'next':
      return Math.min(index + 1, last)
    case 'previous':
      return Math.max(index - 1, 0)
    case 'first':
      return 0
    case 'last':
      return last
  }
}
