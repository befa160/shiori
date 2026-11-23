// Gestes du lecteur paginé : taps et balayages pour tourner, pincement natif du navigateur, zoom
// ×2 au double-clic de souris (lot 10.4b) et déplacement en glissant dans la page zoomée.
import { type MouseEvent, type PointerEvent, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { type Direction, type Move, swipeAction, tapAction } from './controls'

/** Délai sous lequel deux clics font un double-clic (zoom à la souris). */
const DOUBLE_CLICK = 300

/** Pincée par le navigateur (zoom visuel) : la page se déplace au doigt, aucun geste ne la tourne. */
const pinchZoomed = () => (window.visualViewport?.scale ?? 1) > 1.01

/** Gestes sur la vue `index` : `go` tourne la page, `toggleChrome` montre ou masque les commandes.
 * Rend la référence de l'élément qui défile, l'état du zoom, le `touch-action` à poser et les
 * gestionnaires d'événements. */
export function useGestures(index: number, turn: Direction, go: (move: Move) => void, toggleChrome: () => void) {
  const start = useRef<{ x: number; y: number } | null>(null)
  // Pointeurs posés : à deux doigts (pincement pour zoomer), le geste ne tourne jamais la page.
  const pointers = useRef(new Set<number>())
  const pinching = useRef(false)

  // À la souris, ×2 au double-clic sur la vue affichée seulement (tourner la page le quitte), et
  // l'on s'y déplace en glissant. Le point cliqué (fraction de l'écran) reste sous le curseur.
  const [zoom, setZoom] = useState<{ index: number; x: number; y: number } | null>(null)
  const zoomed = zoom?.index === index
  const mainRef = useRef<HTMLElement>(null)
  useLayoutEffect(() => {
    const el = mainRef.current
    if (!el || !zoom || zoom.index !== index) return
    el.scrollLeft = (el.scrollWidth - el.clientWidth) * zoom.x
    el.scrollTop = (el.scrollHeight - el.clientHeight) * zoom.y
  }, [zoom, index])
  useEffect(() => {
    if (!zoomed) return
    const leave = (event: KeyboardEvent) => event.key === 'Escape' && setZoom(null)
    window.addEventListener('keydown', leave)
    return () => window.removeEventListener('keydown', leave)
  }, [zoomed])
  // À la souris, un clic au centre attend de savoir s'il devient un double-clic (zoom).
  const toggleTimer = useRef<number | undefined>(undefined)
  useEffect(() => () => window.clearTimeout(toggleTimer.current), [])
  // Glisser à la souris dans la page zoomée : point de départ et défilement d'alors.
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null)
  // Type du dernier pointeur posé : Chrome sur Android fait un `dblclick` de deux taps quand son
  // zoom au double tap est coupé ; seul un double-clic de souris zoome.
  const pointerType = useRef('mouse')
  // Pincée, la page reçoit aussi le déplacement à un doigt (`manipulation`). Dézoomée, un doigt
  // revient aux gestes du lecteur (taps, balayages) et seul le pincement reste natif.
  const [pinched, setPinched] = useState(pinchZoomed)
  useEffect(() => {
    const viewport = window.visualViewport
    if (!viewport) return
    const update = () => setPinched(pinchZoomed())
    viewport.addEventListener('resize', update)
    return () => viewport.removeEventListener('resize', update)
  }, [])

  // Clic droit ou molette (enregistrer l'image…) : jamais un changement de page.
  const ignored = (event: PointerEvent) => event.pointerType === 'mouse' && event.button !== 0
  const onPointerDown = (event: PointerEvent) => {
    pointerType.current = event.pointerType
    if (ignored(event)) return
    // Panneau Affichage ouvert : ce toucher ne sert qu'à le fermer, jamais à tourner la page.
    if (document.querySelector('[data-radix-popper-content-wrapper]')) {
      start.current = null
      return
    }
    pointers.current.add(event.pointerId)
    if (pointers.current.size > 1) pinching.current = true
    else start.current = { x: event.clientX, y: event.clientY }
    const el = mainRef.current
    if (zoomed && event.pointerType === 'mouse' && el) {
      drag.current = { x: event.clientX, y: event.clientY, left: el.scrollLeft, top: el.scrollTop }
      event.currentTarget.setPointerCapture(event.pointerId)
    }
  }
  const onPointerMove = (event: PointerEvent) => {
    const from = drag.current
    const el = mainRef.current
    if (!from || !el) return
    el.scrollLeft = from.left - (event.clientX - from.x)
    el.scrollTop = from.top - (event.clientY - from.y)
  }
  const onPointerCancel = (event: PointerEvent) => {
    pointers.current.delete(event.pointerId)
    start.current = null
    drag.current = null
  }
  const onPointerUp = (event: PointerEvent) => {
    if (ignored(event)) return
    pointers.current.delete(event.pointerId)
    drag.current = null
    const from = start.current
    start.current = null
    if (pinching.current) {
      if (pointers.current.size === 0) pinching.current = false
      return
    }
    // Zoomée (à la souris ou par pincement) : glisser déplace, cliquer ne tourne pas la page.
    if (!from || zoomed || pinchZoomed()) return
    const to = { x: event.clientX, y: event.clientY }
    const width = event.currentTarget.clientWidth
    if (Math.hypot(to.x - from.x, to.y - from.y) > 10) {
      const swipe = swipeAction(from, to, width, turn)
      if (swipe) go(swipe)
      return
    }
    const tap = tapAction(to.x, width, turn)
    if (tap !== 'toggle') return go(tap)
    // Au doigt, les commandes s'ouvrent tout de suite : il n'y a plus de double tap à attendre.
    if (event.pointerType !== 'mouse') return toggleChrome()
    window.clearTimeout(toggleTimer.current)
    toggleTimer.current = window.setTimeout(toggleChrome, DOUBLE_CLICK)
  }
  // Double-clic au centre : zoom ×2 au point cliqué ; zoomée, n'importe où : retour à la page
  // entière. Sur les côtés, chaque clic a déjà tourné la page.
  const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
    if (pointerType.current !== 'mouse') return
    const width = event.currentTarget.clientWidth || 1
    if (!zoomed && tapAction(event.clientX, width, turn) !== 'toggle') return
    window.clearTimeout(toggleTimer.current)
    const height = event.currentTarget.clientHeight || 1
    setZoom(zoomed ? null : { index, x: event.clientX / width, y: event.clientY / height })
  }

  return {
    mainRef,
    zoomed,
    // Au doigt : le pincement du navigateur, sans son zoom au double tap ; agrandie, le
    // déplacement aussi (lu au début de chaque toucher).
    touchAction: pinched || zoomed ? 'manipulation' : 'pinch-zoom',
    handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel, onDoubleClick },
  } as const
}
