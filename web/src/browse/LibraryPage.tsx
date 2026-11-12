import { useLibraries, useLibrarySeries } from '@/api/queries'
import { count } from './labels'
import { BackLink, QueryState, Row, RowTitle } from './parts'

export function LibraryPage({ id }: { id: number }) {
  const series = useLibrarySeries(id)
  // Le nom vient de la liste des bibliothèques, souvent déjà en cache.
  const name = useLibraries().data?.find((l) => l.id === id)?.name ?? 'Bibliothèque'
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to="/" label="Bibliothèques" />
      <h1 className="line-clamp-2 text-2xl font-semibold break-words">{name}</h1>
      <QueryState
        query={series}
        isEmpty={(list) => list.length === 0}
        empty={<p>Aucune série : la bibliothèque n'a pas encore été scannée, ou son dossier est vide.</p>}
      >
        {(list) => (
          <ul className="space-y-2">
            {list.map((s) => (
              <Row key={s.id} to={`/series/${s.id}`}>
                <RowTitle>{s.name}</RowTitle>
                <p className="text-sm text-muted-foreground">{count(s.volume_count, 'tome', 'tomes')}</p>
              </Row>
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}
