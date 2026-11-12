import { ChevronLeft, ChevronRight } from 'lucide-react'
import { type PointerEvent, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams, useSearchParams } from 'react-router'
import { usePages, useSeries, useVolume } from '@/api/queries'
import { volumeLabel } from '@/browse/labels'
import { QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { useLanguage } from '@/i18n/useLanguage'
import { cn } from '@/lib/utils'
import { NotFound } from '@/pages/NotFound'
import { type Direction, type Move, keyMove, moveIndex, pageUrl, swipeAction, tapAction } from './controls'
import { PageView } from './PageView'

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
  title,
  back,
}: {
  pages: import('@/api/types').PageInfo[]
  direction: Direction
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
  // La page vient de l'adresse (`?page=<id>`, identité stable) : un lien ou un rechargement y revient.
  const index = Math.max(0, pages.findIndex((p) => p.id === params.get('page')))
  const page = pages[index]

  const go = (move: Move | number) => {
    const next = typeof move === 'number' ? move : moveIndex(index, move, pages.length)
    if (next !== index) setParams({ page: pages[next].id }, { replace: true })
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Échap masque les commandes ; Tab les montre (elles sont invisibles, donc hors du parcours
      // clavier) : le clavier atteint tout sans piège.
      if (event.key === 'Escape') return setChrome(false)
      if (event.key === 'Tab' && !chrome) {
        event.preventDefault()
        setChrome(true)
        requestAnimationFrame(() => document.querySelector<HTMLElement>('[role="toolbar"] a, [role="toolbar"] button')?.focus())
        return
      }
      // Dans la barre de commandes, les touches vont au bouton ou au curseur qui a le focus.
      if ((event.target as HTMLElement).closest('[role="toolbar"]')) return
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
      const near = pages[i]
      if (near && near.status === 'ok') new Image().src = pageUrl(near, view)
    }
  }, [index, pages, view])

  // Clic droit ou molette (enregistrer l'image…) : jamais un changement de page.
  const ignored = (event: PointerEvent) => event.pointerType === 'mouse' && event.button !== 0
  const onPointerDown = (event: PointerEvent) => {
    if (ignored(event)) return
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

  const current = t('reader.pageOf', { current: index + 1, total: pages.length })
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
        <PageView key={page.id} page={page} total={pages.length} view={view} />
      </main>
      <p className="sr-only" aria-live="polite">
        {current}
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
        </header>
        <footer className="pointer-events-auto flex items-center gap-3 bg-black/80 px-2 pr-safe pb-safe pl-safe">
          <Button variant="ghost" size="icon" aria-label={t(`reader.${left}`)} onClick={() => go(left)}>
            <ChevronLeft aria-hidden />
          </Button>
          <Slider
            aria-label={t('reader.page')}
            dir={direction}
            min={0}
            max={pages.length - 1}
            step={1}
            value={[dragged ?? index]}
            onValueChange={([value]) => setDragged(value)}
            onValueCommit={([value]) => {
              setDragged(null)
              go(value)
            }}
          />
          <Button variant="ghost" size="icon" aria-label={t(`reader.${right}`)} onClick={() => go(right)}>
            <ChevronRight aria-hidden />
          </Button>
          <span className="shrink-0 pr-2 text-sm text-muted-foreground tabular-nums">
            {number.format((dragged ?? index) + 1)} / {number.format(pages.length)}
          </span>
        </footer>
      </div>
    </>
  )
}
