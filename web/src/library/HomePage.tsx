import { useMutation, useQueryClient } from '@tanstack/react-query'
import { CheckCheck, ChevronRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { api } from '@/api/client'
import { useAllSeries, useContinue, useLibraries, useNewVolumes } from '@/api/queries'
import type { Library, SeriesSummary } from '@/api/types'
import { useIsAdmin } from '@/auth/session'
import { QueryState } from '@/browse/Parts'
import { Button } from '@/components/ui/button'
import { CollectionsShelf } from '@/collections/Collections'
import { toastFailure } from '@/i18n/errors'
import { ContinueCard, CoverSkeletons, NewCard, SeriesCard, Shelf } from './Covers'

// Séries par étagère : au-delà, « Tout voir » mène à la grille de la bibliothèque.
const SHELF_SIZE = 20

/** Accueil : « Continuer la lecture » (toujours en premier), nouveautés des séries suivies,
 * « À lire », collections, puis les derniers ajouts de chaque bibliothèque. */
export function HomePage() {
  const { t } = useTranslation()
  const libraries = useLibraries()
  const series = useAllSeries()
  const reading = useContinue()
  const admin = useIsAdmin()
  return (
    <div className="space-y-8">
      <title>{`${t('home.title')} · Shiori`}</title>
      <h1 className="text-2xl font-semibold">{t('home.title')}</h1>
      {/* Rien à continuer : pas de section vide. */}
      {!(reading.isSuccess && reading.data.length === 0) && (
        <section aria-labelledby="home-continue" className="space-y-3">
          <h2 id="home-continue" className="text-lg font-semibold">
            {t('home.continue')}
          </h2>
          <QueryState query={reading} isEmpty={() => false} empty={null} loading={<CoverSkeletons />}>
            {(items) => (
              <Shelf>
                {items.map((item, i) => (
                  <li key={item.series_id}>
                    <ContinueCard item={item} eager={i < 6} />
                  </li>
                ))}
              </Shelf>
            )}
          </QueryState>
        </section>
      )}
      <NewShelf />
      <ToReadShelf series={series.data ?? []} />
      <CollectionsShelf all={series.data ?? []} />
      <QueryState
        query={libraries}
        isEmpty={(list) => list.length === 0}
        loading={<CoverSkeletons />}
        empty={
          <div className="space-y-3">
            <p>{t(admin ? 'libraries.empty' : 'libraries.emptyReader')}</p>
            {admin && (
              <Button asChild>
                <Link to="/admin">{t('admin.add')}</Link>
              </Button>
            )}
          </div>
        }
      >
        {(list) => (
          <QueryState query={series} isEmpty={() => false} empty={null} loading={<CoverSkeletons />}>
            {(all) => list.map((library) => <LibraryShelf key={library.id} library={library} all={all} />)}
          </QueryState>
        )}
      </QueryState>
    </div>
  )
}

/** Nouveautés (1.3) : rien tant qu'il n'y en a pas, ni squelette (le plus souvent vide). */
function NewShelf() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const items = useNewVolumes().data ?? []
  const seen = useMutation({
    mutationFn: api.markNewSeen,
    onSuccess: () => queryClient.setQueryData(['new'], []),
    onError: toastFailure(t('home.markSeenFailed'), t),
  })
  if (items.length === 0) return null
  return (
    <section aria-labelledby="home-new" className="space-y-3">
      {/* Titre et bouton sur une ligne, aide dessous : sur téléphone, l'aide ne serre pas le bouton. */}
      <div>
        <div className="flex items-center justify-between gap-4">
          <h2 id="home-new" className="text-lg font-semibold">
            {t('home.new')}
          </h2>
          <Button variant="ghost" className="-mr-2 h-11 shrink-0 px-2 text-muted-foreground" disabled={seen.isPending} onClick={() => seen.mutate()}>
            <CheckCheck aria-hidden />
            {t('home.markSeen')}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">{t('home.newHint')}</p>
      </div>
      <Shelf>
        {items.map((item, i) => (
          <li key={item.volume.volume_id}>
            <NewCard item={item} eager={i < 6} />
          </li>
        ))}
      </Shelf>
    </section>
  )
}

/** « À lire » (1.3), dans l'ordre d'ajout ; absente quand la liste est vide. */
function ToReadShelf({ series }: { series: SeriesSummary[] }) {
  const { t } = useTranslation()
  const list = series
    .filter((s) => s.to_read_at)
    .sort((a, b) => (a.to_read_at as string).localeCompare(b.to_read_at as string) || a.id - b.id)
  if (list.length === 0) return null
  return (
    <section aria-labelledby="home-to-read" className="space-y-3">
      <div>
        <h2 id="home-to-read" className="text-lg font-semibold">
          {t('home.toRead')}
        </h2>
        <p className="text-sm text-muted-foreground">{t('home.toReadHint')}</p>
      </div>
      <Shelf>
        {list.map((s) => (
          <li key={s.id}>
            <SeriesCard series={s} />
          </li>
        ))}
      </Shelf>
    </section>
  )
}

function LibraryShelf({ library, all }: { library: Library; all: SeriesSummary[] }) {
  const { t } = useTranslation()
  const recent = all
    .filter((s) => s.library_id === library.id)
    .sort((a, b) => b.created_at.localeCompare(a.created_at) || a.id - b.id)
  const id = `home-library-${library.id}`
  return (
    <section aria-labelledby={id} className="space-y-3">
      <div className="flex items-end justify-between gap-4">
        <div className="min-w-0">
          <h2 id={id} title={library.name} className="truncate text-lg font-semibold">
            {library.name}
          </h2>
          <p className="text-sm text-muted-foreground">{t('home.recent')}</p>
        </div>
        <Link
          to={`/libraries/${library.id}`}
          aria-label={t('home.seeAllNamed', { name: library.name })}
          className="flex h-11 shrink-0 items-center gap-1 rounded-md px-2 text-sm text-muted-foreground transition-colors hover:text-foreground active:bg-accent"
        >
          {t('home.seeAll')}
          <ChevronRight aria-hidden className="size-4" />
        </Link>
      </div>
      {recent.length === 0 ? (
        <p className="rounded-md bg-card p-4 text-sm text-muted-foreground">{t('library.empty')}</p>
      ) : (
        <Shelf>
          {recent.slice(0, SHELF_SIZE).map((s) => (
            <li key={s.id}>
              <SeriesCard series={s} />
            </li>
          ))}
        </Shelf>
      )}
    </section>
  )
}
