import { useTranslation } from 'react-i18next'
import { useLibraries } from '@/api/queries'
import { QueryState, Row, RowTitle } from './parts'

export function LibrariesPage() {
  const { t } = useTranslation()
  const libraries = useLibraries()
  return (
    <section className="space-y-4">
      <title>{`${t('libraries.title')} · Shiori`}</title>
      <h1 className="text-2xl font-semibold">{t('libraries.title')}</h1>
      <QueryState
        query={libraries}
        isEmpty={(list) => list.length === 0}
        empty={
          <>
            <p>{t('libraries.empty')}</p>
            <p className="mt-2 text-sm">{t('libraries.emptyHelp', { route: 'POST /api/libraries' })}</p>
          </>
        }
      >
        {(list) => (
          <ul className="space-y-2">
            {list.map((library) => (
              <Row key={library.id} to={`/libraries/${library.id}`}>
                <RowTitle>{library.name}</RowTitle>
                <p className="text-sm text-muted-foreground">
                  {[
                    t(`kinds.${library.kind}`),
                    t('counts.series', { count: library.series_count }),
                    t('counts.volumes', { count: library.volume_count }),
                  ].join(' · ')}
                </p>
              </Row>
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}
