import { useTranslation } from 'react-i18next'
import { useSeries } from '@/api/queries'
import type { SeriesVolume } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import { volumeLabel } from './labels'
import { BackLink, QueryState, RowTitle } from './parts'

export function SeriesPage({ id }: { id: number }) {
  const { t } = useTranslation()
  const series = useSeries(id)
  const name = series.data?.name ?? t('series.fallbackName')
  return (
    <section className="space-y-4">
      <title>{`${name} · Shiori`}</title>
      <BackLink to={series.data ? `/libraries/${series.data.library_id}` : '/'} label={t('series.back')} />
      <h1 className="line-clamp-2 text-2xl font-semibold break-words">{name}</h1>
      <QueryState
        query={series}
        isEmpty={(s) => s.volumes.length === 0}
        empty={<p>{t('series.empty')}</p>}
        notFound={t('series.notFound')}
      >
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
  const { t, i18n } = useTranslation()
  const label = volumeLabel(volume, t, i18n.language)
  const progress = volume.progress
  return (
    <li className="relative overflow-hidden rounded-md bg-card px-4 py-3">
      <div className="flex min-h-8 items-start gap-3">
        <div className="min-w-0 flex-1">
          <RowTitle>{label}</RowTitle>
          <p className="truncate text-sm text-muted-foreground" title={volume.file_name}>
            {label === volume.file_name ? null : `${volume.file_name} · `}
            {t('counts.pages', { count: volume.page_count })}
          </p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1">
          {volume.status === 'error' && <Badge variant="destructive">{t('volume.statusError')}</Badge>}
          {volume.status === 'warning' && <Badge variant="outline">{t('volume.statusWarning')}</Badge>}
          {progress?.finished && <Badge variant="secondary">{t('volume.read')}</Badge>}
          {progress && !progress.finished && (
            <span className="text-sm text-muted-foreground">
              {new Intl.NumberFormat(i18n.language).format(progress.ordinal + 1)} /{' '}
              {new Intl.NumberFormat(i18n.language).format(progress.page_count)}
            </span>
          )}
        </div>
      </div>
      {progress && !progress.finished && (
        <Progress
          aria-label={t('volume.progress', { label })}
          value={((progress.ordinal + 1) / progress.page_count) * 100}
          className="absolute inset-x-0 bottom-0 h-1 rounded-none"
        />
      )}
    </li>
  )
}
