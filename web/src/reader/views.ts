// Vues du lecteur paginé : ce qui s'affiche à l'écran d'un coup (une page, deux pages, ou une moitié
// de double page). Fonctions pures : toute la logique de disposition est ici, testée par cas.
import type { PageInfo } from '@/api/types'
import type { Direction } from './controls'

export type Layout = 'single' | 'double' | 'auto'
/** Moitié d'une double page, du point de vue de l'image. */
export type Half = 'left' | 'right'

export type View =
  | { kind: 'page'; pages: [PageInfo] | [PageInfo, PageInfo] }
  | { kind: 'half'; page: PageInfo; half: Half }

/** Page plus large que haute : une double page, toujours seule (ou coupée en mode simple). */
export const isWide = (p: PageInfo) => p.width !== null && p.height !== null && p.width > p.height

/** `auto` : double en paysage, simple en portrait (Q13). */
export function effectiveLayout(layout: Layout, view: { width: number; height: number }): 'single' | 'double' {
  if (layout !== 'auto') return layout
  return view.width > view.height ? 'double' : 'single'
}

export interface ViewOptions {
  layout: 'single' | 'double'
  direction: Direction
  /** Double : décale l'appariement d'une page (doubles pages qui tombent mal). */
  offset: boolean
  /** Simple : double page coupée en deux moitiés successives (Q14, par défaut) ou entière. */
  split: boolean
}

export function buildViews(pages: PageInfo[], o: ViewOptions): View[] {
  const views: View[] = []
  if (o.layout === 'single') {
    // Dans le sens de lecture : en manga, la moitié droite d'une double page se lit d'abord.
    const halves: Half[] = o.direction === 'rtl' ? ['right', 'left'] : ['left', 'right']
    for (const page of pages) {
      if (o.split && isWide(page) && page.status === 'ok') {
        for (const half of halves) views.push({ kind: 'half', page, half })
      } else {
        views.push({ kind: 'page', pages: [page] })
      }
    }
    return views
  }
  let pending: PageInfo | null = null
  const flush = () => {
    if (pending) views.push({ kind: 'page', pages: [pending] })
    pending = null
  }
  pages.forEach((page, i) => {
    // La couverture est seule ; avec le décalage, la page suivante aussi.
    const alone = i === 0 || (o.offset && i === 1) || isWide(page)
    if (alone) {
      flush()
      views.push({ kind: 'page', pages: [page] })
    } else if (pending) {
      views.push({ kind: 'page', pages: [pending, page] })
      pending = null
    } else {
      pending = page
    }
  })
  flush()
  return views
}

/** Pages d'une vue, dans l'ordre de lecture. */
export const viewPages = (v: View): PageInfo[] => (v.kind === 'half' ? [v.page] : v.pages)

/** Index de la vue qui montre cette page (et cette moitié si elle est donnée, sinon la première
 * moitié d'une page coupée), 0 sinon. */
export function viewOfPage(views: View[], pageId: string | null, half?: string | null): number {
  const shows = (v: View) => viewPages(v).some((p) => p.id === pageId)
  const exact = views.findIndex((v) => shows(v) && v.kind === 'half' && v.half === half)
  return Math.max(0, exact >= 0 ? exact : views.findIndex(shows))
}

/** Paramètres d'adresse d'une vue : sa première page, et sa moitié pour une page coupée. */
export function viewParams(v: View): Record<string, string> {
  const params: Record<string, string> = { page: viewPages(v)[0].id }
  if (v.kind === 'half') params.half = v.half
  return params
}
