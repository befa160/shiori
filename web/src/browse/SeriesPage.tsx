import { useSeries } from '@/api/queries'
import type { SeriesVolume } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import { count, volumeLabel } from './labels'
import { BackLink, QueryState, RowTitle } from './parts'

export function SeriesPage({ id }: { id: number }) {
  const series = useSeries(id)
  const name = series.data?.name ?? 'Série'
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to={series.data ? `/libraries/${series.data.library_id}` : '/'} label="Séries" />
      <h1 className="line-clamp-2 text-2xl font-semibold break-words">{name}</h1>
      <QueryState query={series} isEmpty={(s) => s.volumes.length === 0} empty={<p>Aucun tome dans cette série.</p>}>
        {(s) => (
          <ul className="space-y-2">
            {s.volumes.map((v) => (
              <VolumeRow key={v.volume_id} volume={v} />
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}

// shortcut: ligne non cliquable tant que le lecteur n'existe pas ; elle deviendra un lien vers /read/:volumeId au lot 3.5.
function VolumeRow({ volume }: { volume: SeriesVolume }) {
  const label = volumeLabel(volume)
  const progress = volume.progress
  return (
    <li className="relative overflow-hidden rounded-md bg-card px-4 py-3">
      <div className="flex min-h-8 items-start gap-3">
        <div className="min-w-0 flex-1">
          <RowTitle>{label}</RowTitle>
          <p className="truncate text-sm text-muted-foreground" title={volume.file_name}>
            {label === volume.file_name ? null : `${volume.file_name} · `}
            {count(volume.page_count, 'page', 'pages')}
          </p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          {volume.status === 'error' && <Badge variant="destructive">Erreur</Badge>}
          {volume.status === 'warning' && <Badge variant="outline">Avertissement</Badge>}
          {progress?.finished && <Badge variant="secondary">Lu</Badge>}
          {progress && !progress.finished && (
            <span className="text-sm text-muted-foreground">
              {progress.ordinal + 1} / {progress.page_count}
            </span>
          )}
        </div>
      </div>
      {progress && !progress.finished && (
        <Progress
          aria-label={`Progression de ${label}`}
          value={((progress.ordinal + 1) / progress.page_count) * 100}
          className="absolute inset-x-0 bottom-0 h-1 rounded-none"
        />
      )}
    </li>
  )
}
