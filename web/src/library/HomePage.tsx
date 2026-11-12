import { ChevronRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { useAllSeries, useContinue, useLibraries } from '@/api/queries'
import type { Library, SeriesSummary } from '@/api/types'
import { QueryState } from '@/browse/parts'
import { ContinueCard, CoverSkeletons, SeriesCard, Shelf } from './covers'

// Séries par étagère : au-delà, « Tout voir » mène à la grille de la bibliothèque.
const SHELF_SIZE = 20

/** Accueil : « Continuer la lecture », puis les derniers ajouts de chaque bibliothèque. */
export function HomePage() {
  const { t } = useTranslation()
  const libraries = useLibraries()
  const series = useAllSeries()
  const reading = useContinue()
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
      <QueryState
        query={libraries}
        isEmpty={(list) => list.length === 0}
        loading={<CoverSkeletons />}
        empty={
          <>
            <p>{t('libraries.empty')}</p>
            <p className="mt-2 text-sm">{t('libraries.emptyHelp', { route: 'POST /api/libraries' })}</p>
          </>
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
