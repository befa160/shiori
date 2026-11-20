import { useQueryClient } from '@tanstack/react-query'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { type MouseEvent, type PointerEvent, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams, useSearchParams } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { usePages, useSeries, useVolume } from '@/api/queries'
import type { PageInfo, PageTurn, ReadingMode, SettingsPatch } from '@/api/types'
import { volumeLabel } from '@/browse/labels'
import { usePatchSeries } from '@/browse/series'
import { QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Toggle } from '@/components/ui/toggle'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'
import { useLanguage } from '@/i18n/useLanguage'
import { useTheme } from '@/lib/theme'
import { cn } from '@/lib/utils'
import { NotFound } from '@/pages/NotFound'
import { type Direction, type Move, keyMove, moveIndex, pageUrl, preloadAhead, swipeAction, tapAction } from './controls'
import { Choice, DisplayOptions, PageSlider, Toolbar } from './chrome'
import { useChromeKeys, useSaveProgress, useViewport } from './hooks'
import { PageView } from './PageView'
import { VerticalPages } from './Vertical'
import { type Layout, type View, buildViews, effectiveLayout, viewOfPage, viewPages, viewParams } from './views'

/** `/read/:volumeId` : l'identifiant d'un tome est un hash de 64 caractères hexadécimaux. */
export function ReaderRoute() {
  const { volumeId = '' } = useParams()
  if (!/^[0-9a-f]{64}$/.test(volumeId)) {
    return (
      <main className="min-h-dvh bg-background p-4 text-foreground">
        <NotFound />
      </main>
    )
  }
  // Une clé par tome : passer au tome suivant repart d'un lecteur neuf (page, zoom, commandes).
  return <Reader key={volumeId} volumeId={volumeId} />
}

/** Ce que les deux modes reçoivent du lecteur. */
export interface ReaderProps {
  volumeId: string
  pages: PageInfo[]
  title: string
  back: string
  /** Page où reprendre sans `?page=` dans l'adresse : la dernière lue, sauf un tome terminé. */
  resume: string | null
  /** Tome suivant de la série, dans son ordre de lecture. */
  next: string | undefined
  onMode: (mode: ReadingMode) => void
  /** Fait d'une page la couverture de la série ; absent si le tome n'est dans aucune série. */
  onCover?: (pageId: string) => void
}

/** Lecteur plein écran, hors du cadre commun : il applique lui-même la langue enregistrée. */
function Reader({ volumeId }: { volumeId: string }) {
  useLanguage()
  useTheme('dark')
  const { t, i18n: i18next } = useTranslation()
  const queryClient = useQueryClient()
  const volume = useVolume(volumeId)
  const pages = usePages(volumeId)
  const seriesId = volume.data?.locations.find((l) => l.series_id !== null)?.series_id ?? null
  const series = useSeries(seriesId)
  const position = series.data?.volumes.findIndex((v) => v.volume_id === volumeId) ?? -1
  const entry = series.data?.volumes[position]
  const next = position >= 0 ? series.data?.volumes[position + 1]?.volume_id : undefined
  const title = [series.data?.name, entry && volumeLabel(entry, t, i18next.language)].filter(Boolean).join(' · ')
  const back = seriesId === null ? '/' : `/series/${seriesId}`
  const cover = usePatchSeries(seriesId ?? 0)
  const onCover =
    seriesId === null
      ? undefined
      : (pageId: string) =>
          cover.mutate(
            { cover_page_id: pageId },
            {
              onSuccess: () => toast.success(t('reader.coverSet')),
              onError: (error) => toast.error(t('reader.coverNotSet'), { description: errorText(error, t) }),
            },
          )
  // Choix de la séance par-dessus les réglages du serveur : un échec d'enregistrement ne les
  // annule pas quand le tome est redemandé au serveur.
  const [chosen, setChosen] = useState<SettingsPatch>({})
  const settings = volume.data && { ...volume.data.settings, ...chosen }
  const progress = volume.data?.progress
  const resume = progress && progress.ordinal < progress.page_count - 1 ? progress.page_id : null

  // Sens et disposition : appliqués tout de suite, retenus pour toute la série (Q12).
  const change = (patch: SettingsPatch) => {
    setChosen((current) => ({ ...current, ...patch }))
    if (seriesId === null) return
    api
      .saveSeriesSettings(seriesId, patch)
      .then(() => {
        void queryClient.invalidateQueries({ queryKey: ['series', seriesId] })
        // Les autres tomes de la série, déjà en cache, reprendront ces réglages à leur ouverture.
        void queryClient.invalidateQueries({ queryKey: ['volumes'], refetchType: 'none' })
      })
      .catch((error: Error) =>
        toast.error(i18n.t('reader.settingsNotSaved'), { id: 'settings', description: errorText(error, i18n.t) }),
      )
  }

  const mode = settings?.reading_mode
  return (
    // Paginé : l'écran fixe ; vertical : le document défile (défilement et clavier natifs).
    // `data-reader` : hors du cadre, pas de barre d'onglets ; les notifications descendent d'autant.
    <div data-reader className={cn('bg-black text-foreground', mode === 'vertical' ? 'min-h-dvh' : 'fixed inset-0')}>
      <title>{`${title || t('reader.page')} · Shiori`}</title>
      <QueryState
        query={pages}
        isEmpty={(list) => list.length === 0}
        notFound={t('reader.notFound')}
        empty={
          <div className="space-y-3">
            <p>{t('reader.empty')}</p>
            <Button asChild variant="secondary">
              <Link to={back}>{t('reader.back')}</Link>
            </Button>
          </div>
        }
      >
        {(list) => {
          // Sans le tome (réglages, progression), on ne sait ni dans quel sens ni où ouvrir.
          if (volume.isPending) return null
          const common = { volumeId, pages: list, title, back, resume, next, onMode: (m: ReadingMode) => change({ reading_mode: m }), onCover }
          return mode === 'vertical' ? (
            <VerticalPages {...common} />
          ) : (
            <Pages
              {...common}
              direction={mode === 'ltr' ? 'ltr' : 'rtl'}
              layout={settings?.page_layout ?? 'auto'}
              onLayout={(layout) => change({ page_layout: layout })}
              pageTurn={settings?.page_turn ?? 'reading'}
              onPageTurn={(turn) => change({ page_turn: turn })}
            />
          )
        }}
      </QueryState>
    </div>
  )
}

/** Délai sous lequel deux clics font un double-clic (zoom à la souris). */
const DOUBLE_CLICK = 300

function Pages({
  volumeId,
  pages,
  direction,
  layout,
  onLayout,
  pageTurn,
  onPageTurn,
  onMode,
  onCover,
  title,
  back,
  resume,
  next,
}: ReaderProps & {
  direction: Direction
  layout: Layout
  onLayout: (layout: Layout) => void
  pageTurn: PageTurn
  onPageTurn: (turn: PageTurn) => void
}) {
  const { t, i18n } = useTranslation()
  const number = new Intl.NumberFormat(i18n.language)
  const view = useViewport()
  const [params, setParams] = useSearchParams()
  const [chrome, setChrome] = useState(false)
  const start = useRef<{ x: number; y: number } | null>(null)
  // Pointeurs posés : à deux doigts (pincement pour zoomer), le geste ne tourne jamais la page.
  const pointers = useRef(new Set<number>())
  const pinching = useRef(false)
  // Décalage et coupe des doubles pages : réglages de la séance, propres à chaque tome lu.
  const [offset, setOffset] = useState(false)
  const [split, setSplit] = useState(true)
  const shown = effectiveLayout(layout, view)
  // Sens des taps, du balayage, des flèches et du curseur (lot 4.11) ; `direction` (le sens de
  // lecture) garde l'ordre des pages dans les vues : en manga, une double page se lit toujours
  // de droite à gauche, même en tournant les pages de gauche à droite.
  const turn: Direction = pageTurn === 'reading' ? direction : pageTurn
  const views = useMemo(
    () => buildViews(pages, { layout: shown, direction, offset, split }),
    [pages, shown, direction, offset, split],
  )
  // La page vient de l'adresse (`?page=<id>`, identité stable) : un lien ou un rechargement y revient.
  const index = viewOfPage(views, params.get('page') ?? resume, params.get('half'))
  const current = views[index]
  const first = viewPages(current)[0]
  const last = viewPages(current).at(-1) ?? first
  const atEnd = index === views.length - 1
  // La dernière page de la vue : atteindre la fin d'un tome le marque terminé.
  useSaveProgress(volumeId, last.id)

  const go = (move: Move | number) => {
    // Au-delà de la dernière page : les commandes, avec le bouton du tome suivant.
    if (move === 'next' && atEnd && next) return setChrome(true)
    const target = typeof move === 'number' ? move : moveIndex(index, move, views.length)
    if (target !== index) setParams(viewParams(views[target]), { replace: true })
  }
  const goToPage = (ordinal: number) => go(viewOfPage(views, pages[ordinal].id))

  useChromeKeys(chrome, setChrome, (event) => {
    const move = keyMove(event.key, event.shiftKey, turn)
    if (move) {
      event.preventDefault()
      go(move)
    }
  })

  // Précharge les vues suivantes (selon la connexion) et la précédente, à la même taille.
  useEffect(() => {
    const ahead = Array.from({ length: preloadAhead() }, (_, i) => index + 1 + i)
    for (const i of [...ahead, index - 1]) {
      const near = views[i]
      if (!near) continue
      for (const p of viewPages(near)) if (p.status === 'ok') new Image().src = pageUrl(p, viewSize(near, view))
    }
  }, [index, views, view])

  // Zoom (lot 10.4b) : au doigt, le pincement natif du navigateur seul ; à la souris, ×2 au
  // double-clic sur la vue affichée seulement (tourner la page le quitte), et l'on s'y déplace en
  // glissant. Le point cliqué (fraction de l'écran) reste sous le curseur.
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
  // Pincée par le navigateur, la page se déplace au doigt : aucun geste ne la tourne.
  const pinchZoomed = () => (window.visualViewport?.scale ?? 1) > 1.01

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
    if (event.pointerType !== 'mouse') return setChrome((shown) => !shown)
    window.clearTimeout(toggleTimer.current)
    toggleTimer.current = window.setTimeout(() => setChrome((shown) => !shown), DOUBLE_CLICK)
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

  const total = pages.length
  const announced =
    current.kind === 'half'
      ? `${t('reader.pageOf', { current: first.ordinal + 1, total })} (${t(`reader.halves.${current.half}`)})`
      : first === last
        ? t('reader.pageOf', { current: first.ordinal + 1, total })
        : t('reader.pagesOf', { first: first.ordinal + 1, last: last.ordinal + 1, total })
  const [left, right] = turn === 'rtl' ? (['next', 'previous'] as const) : (['previous', 'next'] as const)
  return (
    <>
      <main
        ref={mainRef}
        // « safe » : zoomée, la page déborde des deux côtés et reste atteignable en entier.
        className={cn(
          'flex h-full w-full items-center-safe justify-center-safe',
          zoomed ? 'cursor-grab overflow-auto active:cursor-grabbing' : 'overflow-hidden',
        )}
        // Au doigt : le pincement du navigateur, sans son zoom au double tap.
        style={{ touchAction: 'pinch-zoom' }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
        onDoubleClick={onDoubleClick}
      >
        <ViewDisplay
          key={`${first.id}-${current.kind === 'half' ? current.half : viewPages(current).length}`}
          current={current}
          total={total}
          view={zoomed ? { width: view.width * 2, height: view.height * 2 } : view}
          zoomed={zoomed}
          direction={direction}
        />
      </main>
      <p className="sr-only" aria-live="polite">
        {announced}
      </p>
      <Toolbar
        chrome={chrome}
        title={title}
        back={back}
        end={
          atEnd &&
          next && (
            <Button asChild className="shadow-lg">
              <Link to={`/read/${next}`}>{t('reader.nextVolume')}</Link>
            </Button>
          )
        }
        display={
          <DisplayOptions mode={direction} onMode={onMode} onCover={onCover && (() => onCover(first.id))}>
            <Choice
              legend={t('reader.pageTurn')}
              value={pageTurn}
              options={(['reading', 'ltr', 'rtl'] as const).map((p) => [p, t(`reader.pageTurns.${p}`)])}
              onChange={onPageTurn}
              vertical
            />
            <Choice
              legend={t('reader.layout')}
              value={layout}
              options={(['single', 'double', 'auto'] as const).map((l) => [l, t(`reader.layouts.${l}`)])}
              onChange={onLayout}
            />
            {shown === 'double' ? (
              <Toggle pressed={offset} onPressedChange={setOffset} className="w-full">
                {t('reader.offset')}
              </Toggle>
            ) : (
              <Choice
                legend={t('reader.spreads')}
                value={split ? 'split' : 'whole'}
                options={[
                  ['split', t('reader.split')],
                  ['whole', t('reader.whole')],
                ]}
                onChange={(v) => setSplit(v === 'split')}
              />
            )}
          </DisplayOptions>
        }
      >
        <Button variant="ghost" size="icon" aria-label={t(`reader.${left}`)} onClick={() => go(left)}>
          <ChevronLeft aria-hidden />
        </Button>
        <PageSlider
          dir={turn}
          total={total}
          value={first.ordinal}
          onCommit={goToPage}
          counter={(dragged) =>
            `${
              dragged !== null || first === last
                ? number.format((dragged ?? first.ordinal) + 1)
                : `${number.format(first.ordinal + 1)}–${number.format(last.ordinal + 1)}`
            } / ${number.format(total)}`
          }
        />
        <Button variant="ghost" size="icon" aria-label={t(`reader.${right}`)} onClick={() => go(right)}>
          <ChevronRight aria-hidden />
        </Button>
      </Toolbar>
    </>
  )
}

/** Place disponible pour chaque page d'une vue : la moitié de l'écran pour deux pages ; deux
 * écrans de large pour une moitié de double page (l'image entière fait deux fois la moitié). */
function viewSize(v: View, view: { width: number; height: number }) {
  if (v.kind === 'half') return { width: view.width * 2, height: view.height }
  return v.pages.length === 2 ? { width: view.width / 2, height: view.height } : view
}

function ViewDisplay({
  current,
  total,
  view,
  zoomed,
  direction,
}: {
  current: View
  total: number
  view: { width: number; height: number }
  zoomed: boolean
  direction: Direction
}) {
  if (current.kind === 'half') {
    return <PageView page={current.page} total={total} view={view} half={current.half} zoomed={zoomed} />
  }
  const size = viewSize(current, view)
  // Deux pages côte à côte, la première du côté où l'on commence à lire.
  return (
    <div className={cn('flex shrink-0 items-center justify-center', direction === 'rtl' && 'flex-row-reverse')}>
      {current.pages.map((p) => (
        <PageView key={p.id} page={p} total={total} view={size} zoomed={zoomed} />
      ))}
    </div>
  )
}
