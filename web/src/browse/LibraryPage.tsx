import { useTranslation } from 'react-i18next'
import { useLibraries, useLibrarySeries } from '@/api/queries'
import { AlphabetIndex, INDEX_MIN } from '@/library/AlphabetIndex'
import { CoverGrid, CoverSkeletons, SeriesCard } from '@/library/Covers'
import { byName, letterAnchors } from '@/library/filters'
import { BackLink, QueryState } from './Parts'

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
        {(unsorted) => {
          const list = unsorted.toSorted(byName)
          const anchors = list.length >= INDEX_MIN ? letterAnchors(list) : null
          return (
            <>
              <CoverGrid>
                {list.map((s, i) => (
                  <li key={s.id} data-letter={anchors?.get(s.id)} className="scroll-mt-6">
                    {/* Le premier écran se charge tout de suite, le reste au défilement. */}
                    <SeriesCard series={s} eager={i < 12} />
                  </li>
                ))}
              </CoverGrid>
              {anchors && <AlphabetIndex letters={new Set(anchors.values())} />}
            </>
          )
        }}
      </QueryState>
    </section>
  )
}
