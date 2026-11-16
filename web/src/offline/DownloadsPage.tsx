import { useQuery } from '@tanstack/react-query'
import { BookOpen, Trash2, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { cancel, remove, useOffline } from './downloads'
import { pendingPage } from './progress'

/** Tomes gardés sur l'appareil pour lire hors ligne : la page ne demande rien au serveur, elle
 * reste donc utilisable sans réseau (c'est par elle qu'on rouvre un tome hors ligne). */
export function DownloadsPage() {
  const { t, i18n } = useTranslation()
  const { downloads, queue } = useOffline()
  // Place prise par l'application et place disponible, d'après le navigateur.
  const storage = useQuery({
    queryKey: ['storage', downloads.length],
    queryFn: async () => (await navigator.storage?.estimate?.()) ?? null,
  })
  // Mo pour les tomes, Ko pour ce qui ferait « 0 Mo ».
  const size = (bytes: number) => {
    const [unit, value] = bytes < 1e6 ? (['kilobyte', bytes / 1e3] as const) : (['megabyte', bytes / 1e6] as const)
    return new Intl.NumberFormat(i18n.language, { style: 'unit', unit, maximumFractionDigits: 0 }).format(value)
  }
  return (
    <section className="max-w-3xl space-y-6">
      <title>{`${t('offline.title')} · Shiori`}</title>
      <div className="space-y-1">
        <h1 className="text-2xl font-semibold">{t('offline.title')}</h1>
        <p className="text-sm text-muted-foreground">{t('offline.help')}</p>
        {storage.data?.quota ? (
          <p className="text-sm text-muted-foreground">
            {t('offline.storage', { used: size(storage.data.usage ?? 0), free: size(storage.data.quota - (storage.data.usage ?? 0)) })}
          </p>
        ) : null}
      </div>

      {queue.length > 0 && (
        <section aria-labelledby="offline-queue" className="space-y-2">
          <h2 id="offline-queue" className="text-lg font-semibold">
            {t('offline.queue')}
          </h2>
          <ul className="space-y-2">
            {queue.map((job, i) => (
              <li key={job.volumeId} className="flex items-center gap-3 rounded-md bg-card p-3">
                <div className="min-w-0 flex-1 space-y-2">
                  <p className="truncate font-medium">
                    {job.series} · {job.label}
                  </p>
                  {i === 0 ? (
                    <Progress
                      aria-label={t('offline.progress', { label: job.label, done: job.done, total: job.total })}
                      value={job.total ? (job.done / job.total) * 100 : 0}
                      className="h-1"
                    />
                  ) : (
                    <p className="text-sm text-muted-foreground">{t('offline.waiting')}</p>
                  )}
                </div>
                <Button variant="ghost" size="icon" aria-label={t('offline.cancelNamed', { label: job.label })} onClick={() => cancel(job.volumeId)}>
                  <X aria-hidden />
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}

      {downloads.length === 0 && queue.length === 0 ? (
        <p className="rounded-md bg-card p-4 text-muted-foreground">{t('offline.empty')}</p>
      ) : (
        <ul className="space-y-2">
          {downloads.map((d) => {
            const page = pendingPage(d.volumeId)
            return (
              <li key={d.volumeId} className="flex items-center gap-3 rounded-md bg-card p-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium">{d.series}</p>
                  <p className="text-sm text-muted-foreground">
                    {d.label} · {t('counts.pages', { count: d.pages })} · {size(d.bytes)}
                  </p>
                </div>
                <Button asChild variant="secondary" aria-label={t('offline.openNamed', { label: `${d.series} · ${d.label}` })}>
                  {/* Reprise à la page lue hors ligne, que le serveur ne connaît pas encore. */}
                  <Link to={`/read/${d.volumeId}${page ? `?page=${page}` : ''}`}>
                    <BookOpen aria-hidden />
                    <span className="max-sm:sr-only">{t('offline.open')}</span>
                  </Link>
                </Button>
                <Button variant="ghost" size="icon" aria-label={t('offline.removeNamed', { label: `${d.series} · ${d.label}` })} onClick={() => void remove(d.volumeId)}>
                  <Trash2 aria-hidden />
                </Button>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
