import { useLibraries } from '@/api/queries'
import { KIND_LABELS, count } from './labels'
import { QueryState, Row, RowTitle } from './parts'

export function LibrariesPage() {
  const libraries = useLibraries()
  return (
    <section className="space-y-4">
      <title>Bibliothèques · Shiori</title>
      <h1 className="text-2xl font-semibold">Bibliothèques</h1>
      <QueryState
        query={libraries}
        isEmpty={(list) => list.length === 0}
        empty={
          <>
            <p>Aucune bibliothèque pour l'instant.</p>
            <p className="mt-2 text-sm">
              En attendant l'écran d'administration, créez-en une par l'API (
              <code className="text-foreground">POST /api/libraries</code>) puis lancez un scan.
            </p>
          </>
        }
      >
        {(list) => (
          <ul className="space-y-2">
            {list.map((library) => (
              <Row key={library.id} to={`/libraries/${library.id}`}>
                <RowTitle>{library.name}</RowTitle>
                <p className="text-sm text-muted-foreground">
                  {KIND_LABELS[library.kind]} · {count(library.series_count, 'série', 'séries')} ·{' '}
                  {count(library.volume_count, 'tome', 'tomes')}
                </p>
              </Row>
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}
