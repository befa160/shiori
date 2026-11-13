import { useMutation, useQueryClient } from '@tanstack/react-query'
import { EllipsisVertical, Pencil, Play } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useLibraries, useSeries, useVolume } from '@/api/queries'
import type { SeriesDetail, SeriesVolume } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Progress } from '@/components/ui/progress'
import { errorText, issueText } from '@/i18n/errors'
import { Cover, CoverCard, CoverGrid } from '@/library/covers'
import { volumeLabel } from './labels'
import { BackLink, QueryState } from './parts'
import { LISTS } from './series'
import { EditSeriesDialog, SeriesAbout } from './SeriesMetadata'

export function SeriesPage({ id }: { id: number }) {
  const { t } = useTranslation()
  const series = useSeries(id)
  const name = series.data?.name ?? t('series.fallbackName')
  // Retour vers la bibliothèque de la série, nommée (souvent déjà en cache) ; l'accueil sinon.
  const libraryId = series.data?.library_id
  const libraryName = useLibraries().data?.find((l) => l.id === libraryId)?.name ?? t('library.fallbackName')
  // Lien vers un tome (`#volume-…`, depuis les journaux de scan) : React Router ne fait pas défiler
  // jusqu'à l'ancre ; on y va une fois les tomes affichés, et le focus suit pour le clavier.
  const { hash } = useLocation()
  const loaded = series.data !== undefined
  useEffect(() => {
    if (!loaded || !hash) return
    const target = document.getElementById(decodeURIComponent(hash.slice(1)))
    target?.scrollIntoView?.({ block: 'center' })
    target?.querySelector('a')?.focus({ preventScroll: true })
  }, [loaded, hash])
  return (
    <section className="space-y-6">
      <title>{`${name} · Shiori`}</title>
      <BackLink
        to={libraryId === undefined ? '/' : `/libraries/${libraryId}`}
        label={libraryId === undefined ? t('home.title') : libraryName}
      />
      <QueryState query={series} isEmpty={() => false} empty={null} notFound={t('series.notFound')}>
        {(s) => (
          <>
            <SeriesHeader series={s} />
            <section aria-labelledby="series-volumes" className="space-y-3">
              <h2 id="series-volumes" className="text-lg font-semibold">
                {t('series.volumes')}
              </h2>
              {s.volumes.length === 0 ? (
                <p className="rounded-md bg-card p-4 text-muted-foreground">{t('series.empty')}</p>
              ) : (
                <CoverGrid>
                  {s.volumes.map((v, i) => (
                    // Ancre : les journaux de scan mènent directement au tome (`#volume-…`).
                    <li key={v.volume_id} id={`volume-${v.volume_id}`} className="scroll-mt-6">
                      <VolumeCard volume={v} eager={i < 12} />
                    </li>
                  ))}
                </CoverGrid>
              )}
            </section>
          </>
        )}
      </QueryState>
    </section>
  )
}

function SeriesHeader({ series }: { series: SeriesDetail }) {
  const { t, i18n } = useTranslation()
  const total = series.volumes.length
  const read = series.volumes.filter((v) => v.progress?.finished).length
  // Avancement en pages, calculé par le serveur (même règle que la liste des séries).
  const ratio = series.pages_read / Math.max(series.page_count, 1)
  const resume = series.volumes.find((v) => v.volume_id === series.resume_volume_id)
  const cover = series.metadata.cover_page_id ?? series.volumes[0]?.cover_page_id ?? null
  const authors = series.metadata.authors.map((a) => a.name).join(', ')
  const [editing, setEditing] = useState(false)
  return (
    <div className="flex flex-col gap-6 sm:flex-row">
      <div className="relative aspect-[2/3] w-36 shrink-0 overflow-hidden rounded-md bg-card sm:w-48">
        <Cover
          key={series.metadata.cover_image_id ?? cover}
          pageId={cover}
          imageId={series.metadata.cover_image_id}
          title={series.name}
          author={authors}
          eager
        />
      </div>
      <div className="min-w-0 flex-1 space-y-4">
        <h1 title={series.name} className="line-clamp-3 text-2xl font-semibold break-words">
          {series.name}
        </h1>
        {total > 0 && (
          <div className="max-w-sm space-y-2">
            <p className="text-sm text-muted-foreground">
              {t('counts.volumes', { count: total })} · {t('library.readCount', { count: read, total })} ·{' '}
              {t('library.percent', { percent: ratio })}
            </p>
            <Progress aria-label={t('library.percentRead', { percent: ratio })} value={ratio * 100} className="h-1" />
          </div>
        )}
        <div className="flex flex-wrap gap-2">
          {resume && (
            <Button asChild size="lg">
              <Link to={`/read/${resume.volume_id}`}>
                <Play aria-hidden className="fill-current" />
                {t(resumeAction(resume, read === total), { label: volumeLabel(resume, t, i18n.language) })}
              </Link>
            </Button>
          )}
          <Button size="lg" variant="secondary" onClick={() => setEditing(true)}>
            <Pencil aria-hidden />
            {t('series.edit')}
          </Button>
        </div>
        <SeriesAbout series={series} />
        <Dialog open={editing} onOpenChange={setEditing}>
          {editing && <EditSeriesDialog series={series} onDone={() => setEditing(false)} />}
        </Dialog>
      </div>
    </div>
  )
}

/** Libellé du bouton principal : reprendre là où l'on s'est arrêté, relire, ou lire. */
function resumeAction(volume: SeriesVolume, allRead: boolean) {
  const p = volume.progress
  if (p && p.ordinal + 1 < p.page_count) return 'series.resume'
  return allRead ? 'series.reread' : 'series.read'
}

function VolumeCard({ volume, eager }: { volume: SeriesVolume; eager: boolean }) {
  const { t, i18n } = useTranslation()
  const label = volumeLabel(volume, t, i18n.language)
  const p = volume.progress
  const reading = p !== undefined && !p.finished
  // Problème du scan et « Lu » ensemble : l'un n'efface pas l'autre.
  const badges = [
    ...(volume.status === 'error' ? [{ text: t('volume.statusError'), tone: 'error' as const }] : []),
    ...(volume.status === 'warning' ? [{ text: t('volume.statusWarning') }] : []),
    ...(p?.finished ? [{ text: t('volume.read') }] : []),
  ]
  return (
    <CoverCard
      to={`/read/${volume.volume_id}`}
      title={label}
      subtitle={
        reading ? t('home.pageOf', { current: p.ordinal + 1, total: p.page_count }) : t('counts.pages', { count: volume.page_count })
      }
      coverPageId={volume.cover_page_id}
      progress={reading ? (p.ordinal + 1) / p.page_count : null}
      progressLabel={t('volume.progress', { label })}
      badges={badges}
      actions={<VolumeMenu volume={volume} label={label} />}
      eager={eager}
    />
  )
}

/** Actions d'un tome : marquer lu ou non lu, voir les problèmes du dernier scan. */
function VolumeMenu({ volume, label }: { volume: SeriesVolume; label: string }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [issuesOpen, setIssuesOpen] = useState(false)
  const id = volume.volume_id
  const mark = useMutation({
    // « Lu » = la dernière page atteinte, comme dans le lecteur ; « non lu » efface la progression.
    mutationFn: async (read: boolean) => {
      if (!read) return void (await api.clearProgress(id))
      const pages = await api.pages(id)
      await api.saveProgress(id, pages[pages.length - 1].id)
    },
    onSuccess: () => {
      for (const queryKey of [...LISTS, ['volumes', id]]) void queryClient.invalidateQueries({ queryKey })
    },
    onError: (error) => toast.error(t('series.markFailed'), { description: errorText(error, t) }),
  })
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label={t('series.actions', { label })}
          // Pastille de 36 px, zone de toucher de 44 px.
          className="relative flex size-9 items-center justify-center rounded-full bg-black/70 text-foreground transition-colors after:absolute after:-inset-1 hover:bg-black/90"
        >
          <EllipsisVertical aria-hidden className="size-5" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {!volume.progress?.finished && volume.page_count > 0 && (
            <DropdownMenuItem onSelect={() => mark.mutate(true)}>{t('series.markRead')}</DropdownMenuItem>
          )}
          {volume.progress && <DropdownMenuItem onSelect={() => mark.mutate(false)}>{t('series.markUnread')}</DropdownMenuItem>}
          <DropdownMenuItem onSelect={() => setIssuesOpen(true)}>{t('series.issues')}</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <Dialog open={issuesOpen} onOpenChange={setIssuesOpen}>
        {issuesOpen && <IssuesDialog volumeId={id} label={label} file={volume.file_name} />}
      </Dialog>
    </>
  )
}

/** Problèmes du dernier scan d'un tome, traduits d'après leur code stable. */
function IssuesDialog({ volumeId, label, file }: { volumeId: string; label: string; file: string }) {
  const { t } = useTranslation()
  const volume = useVolume(volumeId)
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('series.issuesTitle', { label })}</DialogTitle>
        <DialogDescription className="[overflow-wrap:anywhere]">{t('series.issuesDescription', { file })}</DialogDescription>
      </DialogHeader>
      <QueryState query={volume} isEmpty={(v) => v.issues.length === 0} empty={<p>{t('series.noIssues')}</p>}>
        {(v) => (
          <ul className="space-y-3">
            {v.issues.map((issue, i) => (
              <li key={i} className="rounded-md bg-background p-3">
                <p className={issue.severity === 'error' ? 'font-medium text-destructive' : 'font-medium'}>
                  {t(`severity.${issue.severity}`)} · {issueText(issue, t)}
                </p>
                {issue.entry && <p className="mt-1 font-mono text-xs [overflow-wrap:anywhere] text-muted-foreground">{issue.entry}</p>}
              </li>
            ))}
          </ul>
        )}
      </QueryState>
    </DialogContent>
  )
}
