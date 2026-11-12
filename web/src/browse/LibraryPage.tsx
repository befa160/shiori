import { useTranslation } from 'react-i18next'
import { useLibraries, useLibrarySeries } from '@/api/queries'
import { BackLink, QueryState, Row, RowTitle } from './parts'

export function LibraryPage({ id }: { id: number }) {
  const { t } = useTranslation()
  const series = useLibrarySeries(id)
  // Le nom vient de la liste des bibliothèques, souvent déjà en cache.
  const name = useLibraries().data?.find((l) => l.id === id)?.name ?? t('library.fallbackName')
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to="/" label={t('library.back')} />
      <h1 className="line-clamp-2 text-2xl font-semibold break-words">{name}</h1>
      <QueryState
        query={series}
        isEmpty={(list) => list.length === 0}
        empty={<p>{t('library.empty')}</p>}
        notFound={t('library.notFound')}
      >
        {(list) => (
          <ul className="space-y-2">
            {list.map((s) => (
              <Row key={s.id} to={`/series/${s.id}`}>
                <RowTitle>{s.name}</RowTitle>
                <p className="text-sm text-muted-foreground">{t('counts.volumes', { count: s.volume_count })}</p>
              </Row>
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}
