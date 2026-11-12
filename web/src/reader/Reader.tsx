import { ChevronLeft, ChevronRight, Columns2 } from 'lucide-react'
import { type PointerEvent, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams, useSearchParams } from 'react-router'
import { usePages, useSeries, useVolume } from '@/api/queries'
import { volumeLabel } from '@/browse/labels'
import { QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Slider } from '@/components/ui/slider'
import { Toggle } from '@/components/ui/toggle'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useLanguage } from '@/i18n/useLanguage'
import { cn } from '@/lib/utils'
import { NotFound } from '@/pages/NotFound'
import { type Direction, type Move, keyMove, moveIndex, pageUrl, swipeAction, tapAction } from './controls'
import { PageView } from './PageView'
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
  return <Reader volumeId={volumeId} />
}

function useViewport() {
  const read = () => ({ width: window.innerWidth, height: window.innerHeight })
  const [view, setView] = useState(read)
  useEffect(() => {
    const update = () => setView(read())
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])
  return view
}

/** Lecteur plein écran, hors du cadre commun : il applique lui-même la langue enregistrée. */
function Reader({ volumeId }: { volumeId: string }) {
  useLanguage()
  const { t, i18n } = useTranslation()
  const volume = useVolume(volumeId)
  const pages = usePages(volumeId)
  const seriesId = volume.data?.locations.find((l) => l.series_id !== null)?.series_id ?? null
  const series = useSeries(seriesId)
  const entry = series.data?.volumes.find((v) => v.volume_id === volumeId)
  const title = [series.data?.name, entry && volumeLabel(entry, t, i18n.language)].filter(Boolean).join(' · ')
  const back = seriesId === null ? '/' : `/series/${seriesId}`
  return (
    <div className="fixed inset-0 bg-black text-foreground">
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
        {(list) => (
          <Pages
            pages={list}
            // shortcut: le mode vertical (webtoon) arrive au lot 3.7 ; d'ici là il se lit paginé, de gauche à droite.
            direction={volume.data?.settings.reading_mode === 'rtl' ? 'rtl' : 'ltr'}
            layout={volume.data?.settings.page_layout ?? 'auto'}
            title={title}
            back={back}
          />
        )}
      </QueryState>
    </div>
  )
}

function Pages({
  pages,
  direction,
  layout: initialLayout,
  title,
  back,
}: {
  pages: import('@/api/types').PageInfo[]
  direction: Direction
  layout: Layout
  title: string
  back: string
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
  // Position du curseur pendant qu'on le fait glisser : la page ne change qu'au relâchement,
  // sinon chaque page traversée serait demandée (et encodée) au serveur.
  const [dragged, setDragged] = useState<number | null>(null)
  // Affichage (réglages retenus par série au lot 3.8) : disposition, décalage, coupe des doubles pages.
  const [layout, setLayout] = useState<Layout>(initialLayout)
  const [offset, setOffset] = useState(false)
  const [split, setSplit] = useState(true)
  const shown = effectiveLayout(layout, view)
  const views = useMemo(
    () => buildViews(pages, { layout: shown, direction, offset, split }),
    [pages, shown, direction, offset, split],
  )
  // La page vient de l'adresse (`?page=<id>`, identité stable) : un lien ou un rechargement y revient.
  const index = viewOfPage(views, params.get('page'), params.get('half'))
  const current = views[index]
  const first = viewPages(current)[0]
  const last = viewPages(current).at(-1) ?? first

  const go = (move: Move | number) => {
    const next = typeof move === 'number' ? move : moveIndex(index, move, views.length)
    if (next !== index) setParams(viewParams(views[next]), { replace: true })
  }
  const goToPage = (ordinal: number) => go(viewOfPage(views, pages[ordinal].id))

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Échap masque les commandes ; Tab les montre (elles sont invisibles, donc hors du parcours
      // clavier) : le clavier atteint tout sans piège.
      const target = event.target as HTMLElement
      // Panneau Affichage ouvert : ses touches lui appartiennent (Échap le ferme, sans plus).
      if (target.closest('[data-radix-popper-content-wrapper]')) return
      if (event.key === 'Escape') {
        // Le focus quitte la barre qui devient invisible : les flèches retournent aux pages.
        if (target.closest('[role="toolbar"]')) target.blur()
        return setChrome(false)
      }
      if (event.key === 'Tab' && !chrome) {
        event.preventDefault()
        setChrome(true)
        requestAnimationFrame(() => document.querySelector<HTMLElement>('[role="toolbar"] a, [role="toolbar"] button')?.focus())
        return
      }
      // Dans la barre de commandes, les touches vont au bouton ou au curseur qui a le focus.
      if (target.closest('[role="toolbar"]')) return
      const move = keyMove(event.key, event.shiftKey, direction)
      if (move) {
        event.preventDefault()
        go(move)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  // Précharge les deux pages suivantes et la précédente, à la même taille.
  useEffect(() => {
    for (const i of [index + 1, index + 2, index - 1]) {
      const near = views[i]
      if (!near) continue
      for (const p of viewPages(near)) if (p.status === 'ok') new Image().src = pageUrl(p, viewSize(near, view))
    }
  }, [index, views, view])

  // Clic droit ou molette (enregistrer l'image…) : jamais un changement de page.
  const ignored = (event: PointerEvent) => event.pointerType === 'mouse' && event.button !== 0
  const onPointerDown = (event: PointerEvent) => {
    if (ignored(event)) return
    // Panneau Affichage ouvert : ce toucher ne sert qu'à le fermer, jamais à tourner la page.
    if (document.querySelector('[data-radix-popper-content-wrapper]')) {
      start.current = null
      return
    }
    pointers.current.add(event.pointerId)
    if (pointers.current.size > 1) pinching.current = true
    else start.current = { x: event.clientX, y: event.clientY }
  }
  const onPointerCancel = (event: PointerEvent) => {
    pointers.current.delete(event.pointerId)
    start.current = null
  }
  const onPointerUp = (event: PointerEvent) => {
    if (ignored(event)) return
    pointers.current.delete(event.pointerId)
    const from = start.current
    start.current = null
    if (pinching.current) {
      if (pointers.current.size === 0) pinching.current = false
      return
    }
    if (!from) return
    const to = { x: event.clientX, y: event.clientY }
    const width = event.currentTarget.clientWidth
    if (Math.hypot(to.x - from.x, to.y - from.y) > 10) {
      const swipe = swipeAction(from, to, width, direction)
      if (swipe) go(swipe)
      return
    }
    const tap = tapAction(to.x, width, direction)
    if (tap === 'toggle') setChrome((shown) => !shown)
    else go(tap)
  }

  const total = pages.length
  const announced =
    current.kind === 'half'
      ? `${t('reader.pageOf', { current: first.ordinal + 1, total })} (${t(`reader.halves.${current.half}`)})`
      : first === last
        ? t('reader.pageOf', { current: first.ordinal + 1, total })
        : t('reader.pagesOf', { first: first.ordinal + 1, last: last.ordinal + 1, total })
  const [left, right] = direction === 'rtl' ? (['next', 'previous'] as const) : (['previous', 'next'] as const)
  return (
    <>
      <main
        className="flex h-full w-full items-center justify-center overflow-hidden"
        style={{ touchAction: 'pinch-zoom' }}
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
      >
        <ViewDisplay key={`${first.id}-${current.kind === 'half' ? current.half : viewPages(current).length}`} current={current} total={total} view={view} direction={direction} />
      </main>
      <p className="sr-only" aria-live="polite">
        {announced}
      </p>
      <div
        role="toolbar"
        aria-label={t('reader.controls')}
        className={cn('pointer-events-none fixed inset-0 flex flex-col justify-between', !chrome && 'invisible')}
      >
        <header className="pointer-events-auto flex items-center gap-2 bg-black/80 px-2 pt-safe pr-safe pl-safe">
          <Button asChild variant="ghost">
            <Link to={back}>
              <ChevronLeft aria-hidden />
              {t('reader.back')}
            </Link>
          </Button>
          <p className="min-w-0 flex-1 truncate font-medium">{title}</p>
          <DisplayOptions
            layout={layout}
            shown={shown}
            offset={offset}
            split={split}
            onLayout={setLayout}
            onOffset={setOffset}
            onSplit={setSplit}
          />
        </header>
        <footer className="pointer-events-auto flex items-center gap-3 bg-black/80 px-2 pr-safe pb-safe pl-safe">
          <Button variant="ghost" size="icon" aria-label={t(`reader.${left}`)} onClick={() => go(left)}>
            <ChevronLeft aria-hidden />
          </Button>
          <Slider
            aria-label={t('reader.page')}
            dir={direction}
            min={0}
            max={total - 1}
            step={1}
            value={[dragged ?? first.ordinal]}
            onValueChange={([value]) => setDragged(value)}
            onValueCommit={([value]) => {
              setDragged(null)
              goToPage(value)
            }}
          />
          <Button variant="ghost" size="icon" aria-label={t(`reader.${right}`)} onClick={() => go(right)}>
            <ChevronRight aria-hidden />
          </Button>
          <span className="shrink-0 pr-2 text-sm text-muted-foreground tabular-nums">
            {dragged !== null || first === last
              ? number.format((dragged ?? first.ordinal) + 1)
              : `${number.format(first.ordinal + 1)}–${number.format(last.ordinal + 1)}`}{' '}
            / {number.format(total)}
          </span>
        </footer>
      </div>
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
  direction,
}: {
  current: View
  total: number
  view: { width: number; height: number }
  direction: Direction
}) {
  if (current.kind === 'half') {
    return <PageView page={current.page} total={total} view={view} half={current.half} />
  }
  const size = viewSize(current, view)
  // Deux pages côte à côte, la première du côté où l'on commence à lire.
  return (
    <div className={cn('flex items-center justify-center', direction === 'rtl' && 'flex-row-reverse')}>
      {current.pages.map((p) => (
        <PageView key={p.id} page={p} total={total} view={size} />
      ))}
    </div>
  )
}

function DisplayOptions({
  layout,
  shown,
  offset,
  split,
  onLayout,
  onOffset,
  onSplit,
}: {
  layout: Layout
  shown: 'single' | 'double'
  offset: boolean
  split: boolean
  onLayout: (layout: Layout) => void
  onOffset: (offset: boolean) => void
  onSplit: (split: boolean) => void
}) {
  const { t } = useTranslation()
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="ghost">
          <Columns2 aria-hidden />
          {t('reader.display')}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-72 space-y-4">
        <fieldset className="space-y-2">
          <legend className="text-sm text-muted-foreground">{t('reader.layout')}</legend>
          <ToggleGroup
            type="single"
            value={layout}
            onValueChange={(value) => value && onLayout(value as Layout)}
            className="w-full"
          >
            {(['single', 'double', 'auto'] as const).map((l) => (
              <ToggleGroupItem key={l} value={l} className="flex-1">
                {t(`reader.layouts.${l}`)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </fieldset>
        {shown === 'double' ? (
          <Toggle pressed={offset} onPressedChange={onOffset} className="w-full">
            {t('reader.offset')}
          </Toggle>
        ) : (
          <fieldset className="space-y-2">
            <legend className="text-sm text-muted-foreground">{t('reader.spreads')}</legend>
            <ToggleGroup
              type="single"
              value={split ? 'split' : 'whole'}
              onValueChange={(value) => value && onSplit(value === 'split')}
              className="w-full"
            >
              <ToggleGroupItem value="split" className="flex-1">
                {t('reader.split')}
              </ToggleGroupItem>
              <ToggleGroupItem value="whole" className="flex-1">
                {t('reader.whole')}
              </ToggleGroupItem>
            </ToggleGroup>
          </fieldset>
        )}
      </PopoverContent>
    </Popover>
  )
}
