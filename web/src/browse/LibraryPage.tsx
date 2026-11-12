import { useTranslation } from 'react-i18next'
import { useLibraries, useLibrarySeries } from '@/api/queries'
import { CoverGrid, CoverSkeletons, SeriesCard } from '@/library/covers'
import { BackLink, QueryState } from './parts'

export function LibraryPage({ id }: { id: number }) {
  const { t } = useTranslation()
  const series = useLibrarySeries(id)
  // Le nom vient de la liste des bibliothèques, souvent déjà en cache.
  const name = useLibraries().data?.find((l) => l.id === id)?.name ?? t('library.fallbackName')
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to="/" label={t('home.title')} />
      <h1 title={name} className="line-clamp-2 text-2xl font-semibold break-words">
        {name}
      </h1>
      <QueryState
        query={series}
        isEmpty={(list) => list.length === 0}
        empty={<p>{t('library.empty')}</p>}
        notFound={t('library.notFound')}
        loading={<CoverSkeletons grid />}
      >
        {(list) => (
          <CoverGrid>
            {list.map((s, i) => (
              <li key={s.id}>
                {/* Le premier écran se charge tout de suite, le reste au défilement. */}
                <SeriesCard series={s} eager={i < 12} />
              </li>
            ))}
          </CoverGrid>
        )}
      </QueryState>
    </section>
  )
}
