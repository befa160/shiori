import { Play } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { pageImageUrl } from '@/api/client'
import type { ContinueItem, SeriesSummary } from '@/api/types'
import { volumeLabel } from '@/browse/labels'
import { cn } from '@/lib/utils'
import { titleHue } from './titleHue'
import { Skeleton } from '@/components/ui/skeleton'

// Cartes de couverture (DESIGN_SYSTEM.md, « Carte de couverture », « Couverture de repli »).

const THUMBS = [160, 320, 480] as const
// Largeur affichée : la moitié de l'écran sur mobile (deux colonnes), 12 rem au plus ailleurs.
const SIZES = '(min-width: 640px) 12rem, 45vw'

/** Couverture générée : dégradé sombre (16 % de luminosité au plus, contraste vérifié) et titre. */
export function FallbackCover({ title }: { title: string }) {
  return (
    <div
      aria-hidden
      data-testid="fallback-cover"
      className="flex size-full items-center justify-center p-3"
      style={{ background: `linear-gradient(to bottom, hsl(${titleHue(title)} 35% 16%), #121212)` }}
    >
      <span className="line-clamp-4 text-center font-semibold break-words text-foreground">{title}</span>
    </div>
  )
}

/** Miniature JPEG du serveur ; couverture de repli si elle manque ou ne se charge pas. */
export function Cover({ pageId, title, eager }: { pageId: string | null; title: string; eager: boolean }) {
  const [failed, setFailed] = useState(false)
  if (pageId === null || failed) return <FallbackCover title={title} />
  return (
    <img
      src={pageImageUrl(pageId, { width: 320, format: 'jpeg' })}
      srcSet={THUMBS.map((width) => `${pageImageUrl(pageId, { width, format: 'jpeg' })} ${width}w`).join(', ')}
      sizes={SIZES}
      alt=""
      loading={eager ? 'eager' : 'lazy'}
      decoding="async"
      onError={() => setFailed(true)}
      className="size-full object-cover transition-transform duration-200 ease-out group-hover:scale-[1.03]"
    />
  )
}

export interface CoverCardProps {
  to: string
  title: string
  subtitle: string
  coverPageId: string | null
  /** Barre ambre en bas de la couverture, de 0 à 1 ; absente si `null`. */
  progress: number | null
  progressLabel?: string
  /** Pastille en haut à gauche ; `error` sur fond rouge foncé (le mot porte le sens). */
  badge?: { text: string; tone?: 'error' }
  /** Menu d'actions en haut à droite de la couverture, toujours visible (toucher). */
  actions?: ReactNode
  /** Bouton « Lire » au survol (bureau) et au clavier. */
  quickRead?: { to: string; label: string }
  /** Couvertures du premier écran : chargées tout de suite. */
  eager?: boolean
}

export function CoverCard({ to, title, subtitle, coverPageId, progress, progressLabel, badge, actions, quickRead, eager = false }: CoverCardProps) {
  return (
    <div className="group relative">
      {/* Texte d'abord dans le DOM (le lecteur d'écran annonce le titre en premier), couverture
          au-dessus à l'écran. */}
      <Link to={to} className="flex flex-col-reverse rounded-md active:brightness-90">
        <div className="mt-2">
          <p title={title} className="line-clamp-2 text-sm font-medium break-words">
            {title}
          </p>
          <p className="line-clamp-1 text-sm text-muted-foreground">{subtitle}</p>
        </div>
        <div className="relative aspect-[2/3] overflow-hidden rounded-md bg-card ring-1 ring-transparent transition-shadow group-hover:ring-white/15">
          {/* `key` : une autre couverture repart sans l'échec de la précédente. */}
          <Cover key={coverPageId} pageId={coverPageId} title={title} eager={eager} />
          {badge && (
            <span
              className={cn(
                // Tronquée avant le menu d'actions (en haut à droite) sur une carte étroite.
                'absolute top-2 left-2 max-w-[calc(100%-3.5rem)] truncate rounded-full px-2 py-0.5 text-xs font-medium',
                badge.tone === 'error' ? 'bg-destructive-fill text-white' : 'bg-black/80 text-foreground',
              )}
            >
              {badge.text}
            </span>
          )}
          {progress !== null && (
            <div
              role="progressbar"
              aria-label={progressLabel}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(progress * 100)}
              className="absolute inset-x-0 bottom-0 h-1 bg-black/80"
            >
              <div className="h-full bg-primary" style={{ width: `${progress * 100}%` }} />
            </div>
          )}
        </div>
      </Link>
      {actions && <div className="absolute top-1 right-1">{actions}</div>}
      {quickRead && (
        // Calque de la taille de la couverture, pour poser le bouton en bas à droite de l'image.
        <div className="pointer-events-none absolute inset-x-0 top-0 aspect-[2/3]">
          <Link
            to={quickRead.to}
            aria-label={quickRead.label}
            title={quickRead.label}
            className="pointer-events-auto absolute right-2 bottom-3 hidden size-11 items-center justify-center rounded-full bg-primary text-primary-foreground opacity-0 shadow-md transition-opacity duration-150 group-hover:opacity-100 focus-visible:opacity-100 [@media(hover:hover)]:flex"
          >
            <Play aria-hidden className="size-5 fill-current" />
          </Link>
        </div>
      )}
    </div>
  )
}

/** Grille de couvertures, sans breakpoint (DESIGN_SYSTEM.md, « Grille de couvertures »). */
export function CoverGrid({ children }: { children: ReactNode }) {
  return <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3 lg:gap-4">{children}</ul>
}

/** Étagère défilante : défilement natif accroché aux cartes, sans barre visible, jusqu'aux bords. */
export function Shelf({ children }: { children: ReactNode }) {
  return (
    <ul className="scrollbar-none -mx-4 flex snap-x snap-mandatory scroll-px-4 gap-3 overflow-x-auto px-4 pb-1 lg:-mx-8 lg:scroll-px-8 lg:gap-4 lg:px-8 [&>li]:w-36 [&>li]:shrink-0 [&>li]:snap-start lg:[&>li]:w-40">
      {children}
    </ul>
  )
}

/** Chargement : des couvertures vides à la forme des vraies. */
export function CoverSkeletons({ grid = false }: { grid?: boolean }) {
  const { t } = useTranslation()
  const List = grid ? CoverGrid : Shelf
  return (
    <div aria-busy="true">
      <span className="sr-only">{t('common.loading')}</span>
      <List>
        {Array.from({ length: grid ? 12 : 6 }, (_, i) => (
          <li key={i}>
            <Skeleton className="aspect-[2/3] rounded-md bg-card" />
            <Skeleton className="mt-2 h-4 w-3/4 bg-card" />
          </li>
        ))}
      </List>
    </div>
  )
}

/** Une série : progression en tomes lus, « Lire » ouvre le tome à reprendre. */
export function SeriesCard({ series, eager }: { series: SeriesSummary; eager?: boolean }) {
  const { t } = useTranslation()
  const resume = series.resume_volume_id
  return (
    <CoverCard
      to={`/series/${series.id}`}
      title={series.name}
      subtitle={t('counts.volumes', { count: series.volume_count })}
      coverPageId={series.cover_page_id}
      progress={series.reading_status === 'in_progress' ? series.read_count / Math.max(series.volume_count, 1) : null}
      progressLabel={t('library.readCount', { count: series.read_count, total: series.volume_count })}
      badge={series.reading_status === 'read' ? { text: t('volume.read') } : undefined}
      quickRead={resume ? { to: `/read/${resume}`, label: t('library.readNamed', { name: series.name }) } : undefined}
      eager={eager}
    />
  )
}

/** Une entrée de « Continuer la lecture » : le tome en cours (avec sa page) ou le suivant. */
export function ContinueCard({ item, eager }: { item: ContinueItem; eager?: boolean }) {
  const { t, i18n } = useTranslation()
  const { volume } = item
  const label = volumeLabel(volume, t, i18n.language)
  const progress = volume.progress
  return (
    <CoverCard
      to={`/read/${volume.volume_id}`}
      title={item.series_name}
      subtitle={progress ? `${label} · ${t('home.pageOf', { current: progress.ordinal + 1, total: progress.page_count })}` : label}
      coverPageId={volume.cover_page_id}
      progress={progress ? (progress.ordinal + 1) / progress.page_count : null}
      progressLabel={t('volume.progress', { label })}
      badge={progress ? undefined : { text: t('home.next') }}
      eager={eager}
    />
  )
}
