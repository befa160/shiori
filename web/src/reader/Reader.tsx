import { useQueryClient } from '@tanstack/react-query'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams, useSearchParams } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { usePages, useSeries, useVolume } from '@/api/queries'
import type { PageInfo, PageTurn, ReadingMode, SettingsPatch } from '@/api/types'
import { volumeLabel } from '@/browse/labels'
import { usePatchSeries } from '@/browse/series'
import { QueryState } from '@/browse/Parts'
import { Button } from '@/components/ui/button'
import { Toggle } from '@/components/ui/toggle'
import i18n from '@/i18n'
import { errorText, toastFailure } from '@/i18n/errors'
import { useLanguage } from '@/i18n/useLanguage'
import { useTheme } from '@/lib/theme'
import { cn } from '@/lib/utils'
import { NotFound } from '@/pages/NotFound'
import { type Direction, type Move, keyMove, moveIndex, pageUrl, preloadAhead } from './controls'
import { useBrightness } from './brightness'
import { useGestures } from './gestures'
import { Choice, DisplayOptions, PageSlider, Toolbar } from './Chrome'
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
  /** Tome suivant de la série, dans son ordre de lecture, et le texte de son bouton : un tome en
   * erreur au dernier scan est proposé quand même mais signalé (jamais sauté, principe 4). */
  next: { id: string; label: string } | undefined
  onMode: (mode: ReadingMode) => void
  /** Fait d'une page la couverture de la série ; absent si le tome n'est dans aucune série. */
  onCover?: (pageId: string) => void
  /** Luminosité des pages en pourcentage, retenue par appareil. */
  brightness: number
  onBrightness: (value: number) => void
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
  const following = position >= 0 ? series.data?.volumes[position + 1] : undefined
  const next = following && {
    id: following.volume_id,
    label: t(following.status === 'error' ? 'reader.nextVolumeBroken' : 'reader.nextVolume'),
  }
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
              onError: toastFailure(t('reader.coverNotSet'), t),
            },
          )
  // Choix de la séance par-dessus les réglages du serveur : un échec d'enregistrement ne les
  // annule pas quand le tome est redemandé au serveur.
  const [chosen, setChosen] = useState<SettingsPatch>({})
  const [brightness, onBrightness] = useBrightness()
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
          const common = { volumeId, pages: list, title, back, resume, next, onMode: (m: ReadingMode) => change({ reading_mode: m }), onCover, brightness, onBrightness }
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
      {/* Voile au-dessus des pages, sous les commandes (z-20) : un `filter` serait recalculé à
          chaque image ; le voile ne laisse passer ni clic ni geste. */}
      {brightness < 100 && (
        <div aria-hidden className="pointer-events-none fixed inset-0 z-10 bg-black" style={{ opacity: 1 - brightness / 100 }} />
      )}
    </div>
  )
}

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
  brightness,
  onBrightness,
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

  const { mainRef, zoomed, touchAction, handlers } = useGestures(index, turn, go, () => setChrome((shown) => !shown))

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
        style={{ touchAction }}
        {...handlers}
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
              <Link to={`/read/${next.id}`}>{next.label}</Link>
            </Button>
          )
        }
        display={
          <DisplayOptions mode={direction} onMode={onMode} onCover={onCover && (() => onCover(first.id))} brightness={brightness} onBrightness={onBrightness}>
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
