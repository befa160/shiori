import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowDown, ArrowUp, Loader2 } from 'lucide-react'
import { useEffect, useId, useRef } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { Settings, SourceId, SourceIssue } from '@/api/types'
import { QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Progress } from '@/components/ui/progress'
import { errorText } from '@/i18n/errors'

const CREDITS: [string, string][] = [
  ['AniList', 'https://anilist.co'],
  ['MangaDex', 'https://mangadex.org'],
]

/** Métadonnées web (lot 5.7a) : activation expliquée, ordre des sources, identification et ce
 * qui reste à vérifier ; crédits des sources (MangaDex l'exige). */
export function AdminMetadata() {
  const { t, i18n } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  // Chaque seconde pendant une identification ; toutes les 10 s au repos, pour voir celle qu'un
  // scan lance à la fin (depuis n'importe quel appareil).
  const overview = useQuery({
    queryKey: ['admin', 'metadata'],
    queryFn: ({ signal }) => api.metadataOverview({ signal }),
    refetchInterval: (query) => (query.state.data?.identify.running ? 1000 : 10_000),
  })
  // Une identification terminée a changé titres, auteurs et statuts : les listes sont redemandées.
  const running = overview.data?.identify.running
  const wasRunning = useRef(running)
  useEffect(() => {
    if (wasRunning.current && running === false) {
      for (const queryKey of [['series'], ['libraries'], ['continue']]) void queryClient.invalidateQueries({ queryKey })
    }
    wasRunning.current = running
  }, [running, queryClient])
  const settings = useQuery({ queryKey: ['settings'], queryFn: ({ signal }) => api.settings({ signal }) })
  const save = useMutation({
    mutationFn: (patch: Partial<Settings>) => api.saveSettings(patch),
    onSuccess: (saved) => {
      queryClient.setQueryData(['settings'], saved)
      // L'ordre change les titres et résumés venus du web.
      for (const queryKey of [['admin', 'metadata'], ['series'], ['libraries'], ['continue']]) {
        void queryClient.invalidateQueries({ queryKey })
      }
    },
    onError: (error) => toast.error(t('admin.metadataNotSaved'), { description: errorText(error, t) }),
  })
  const identify = useMutation({
    mutationFn: api.identify,
    onSuccess: () => {
      toast.success(t('admin.identifyStarted'))
      void queryClient.invalidateQueries({ queryKey: ['admin', 'metadata'] })
    },
    onError: (error) => toast.error(t('admin.identifyNotStarted'), { description: errorText(error, t) }),
  })
  const online = save.isPending && save.variables.metadata_online !== undefined ? save.variables.metadata_online : settings.data?.metadata_online
  const n = new Intl.NumberFormat(i18n.language)
  const name = (source: SourceId) => t(`admin.sourceNames.${source}`)
  return (
    <section aria-labelledby={`${id}-title`} className="max-w-prose space-y-4 rounded-md bg-card p-4">
      <h2 id={`${id}-title`} className="text-lg font-semibold">
        {t('admin.metadata')}
      </h2>
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {t('admin.metadataHelp')}
      </p>
      {online !== undefined && (
        <div className="flex items-center gap-3">
          <Checkbox
            id={`${id}-online`}
            checked={online}
            onCheckedChange={(v) => save.mutate({ metadata_online: v === true })}
            aria-describedby={`${id}-help`}
          />
          <label htmlFor={`${id}-online`} className="flex min-h-11 items-center text-sm font-medium">
            {t('admin.metadataOnline')}
          </label>
        </div>
      )}
      <QueryState query={overview} isEmpty={() => false} empty={null}>
        {(o) => {
          const move = (from: number, to: number) => {
            const order = [...o.sources]
            ;[order[from], order[to]] = [order[to], order[from]]
            save.mutate({ metadata_sources: order })
          }
          const { running, done, total, stopped } = o.identify
          return (
            <>
              <div className="space-y-2">
                <h3 className="text-sm font-medium">{t('admin.sources')}</h3>
                <p className="text-sm text-muted-foreground">
                  {t('admin.sourcesHelp')} {settings.data?.metadata_sources === null && t('admin.sourcesDefault')}
                </p>
                {/* Monter, descendre : pas de glisser-déposer obligatoire (WCAG 2.2). */}
                <ol className="space-y-2">
                  {o.sources.map((source, i) => (
                    <li key={source} className="flex min-h-11 items-center gap-2 rounded-md bg-background px-3">
                      <span className="w-6 text-sm text-muted-foreground tabular-nums">{i + 1}.</span>
                      <span className="flex-1 font-medium">{name(source)}</span>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={t('admin.sourceUp', { source: name(source) })}
                        disabled={i === 0 || save.isPending}
                        onClick={() => move(i, i - 1)}
                      >
                        <ArrowUp aria-hidden />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={t('admin.sourceDown', { source: name(source) })}
                        disabled={i === o.sources.length - 1 || save.isPending}
                        onClick={() => move(i, i + 1)}
                      >
                        <ArrowDown aria-hidden />
                      </Button>
                    </li>
                  ))}
                </ol>
                {settings.data?.metadata_sources && (
                  <Button variant="link" className="h-11 px-0" onClick={() => save.mutate({ metadata_sources: null })}>
                    {t('admin.sourcesReset')}
                  </Button>
                )}
              </div>
              <div className="space-y-2">
                <Button variant="secondary" disabled={!o.online || running || identify.isPending} onClick={() => identify.mutate()}>
                  {(running || identify.isPending) && <Loader2 aria-hidden className="animate-spin" />}
                  {t('admin.identify')}
                </Button>
                <p className="text-sm text-muted-foreground">{t('admin.identifyHelp')}</p>
                <div role="status" className="space-y-2">
                  {running && total > 0 && (
                    <>
                      <Progress aria-label={t('admin.identify')} value={(done / total) * 100} className="h-2" />
                      <p className="text-sm">{t('admin.identifyProgress', { done: n.format(done), total: n.format(total) })}</p>
                    </>
                  )}
                  {!running && stopped && <p className="text-sm text-destructive">{t(stoppedKey(stopped))}</p>}
                  {!running && !stopped && total > 0 && (
                    <p className="text-sm text-muted-foreground">{t('admin.identifyDone', { done: n.format(done) })}</p>
                  )}
                </div>
              </div>
              <Issues issues={o.issues} name={name} />
            </>
          )
        }}
      </QueryState>
      <p className="text-sm text-muted-foreground">
        {t('admin.credits')}{' '}
        {CREDITS.map(([label, href], i) => (
          <span key={href}>
            {i > 0 && ', '}
            <a href={href} target="_blank" rel="noreferrer noopener" className="underline underline-offset-4">
              {label}
            </a>
          </span>
        ))}
        .
      </p>
    </section>
  )
}

const LAST_ERRORS = ['offline', 'rate_limited', 'not_found', 'http_error', 'bad_response'] as const

/** Dernier échec d'une source, d'après son code stable ; un code inconnu reste tel quel. */
function lastError(code: string, t: TFunction) {
  const known = LAST_ERRORS.find((c) => c === code)
  return known ? t(`admin.lastError.${known}`) : code
}

function stoppedKey(code: string) {
  return code === 'rate_limited' || code === 'offline' ? (`admin.identifyStopped.${code}` as const) : 'admin.identifyStopped.internal'
}

/** Séries dont une source demande l'attention ; chacune mène à sa page (où l'identifier). */
function Issues({ issues, name }: { issues: SourceIssue[]; name: (s: SourceId) => string }) {
  const { t } = useTranslation()
  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium">{t('admin.toCheck')}</h3>
      {issues.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('admin.toCheckEmpty')}</p>
      ) : (
        <ul className="divide-y divide-border">
          {issues.map((issue) => (
            <li key={`${issue.series_id}-${issue.source}`}>
              <Link to={`/series/${issue.series_id}`} className="flex min-h-11 flex-wrap items-center gap-x-3 py-2 hover:underline">
                <span className="min-w-0 flex-1 truncate font-medium">{issue.series_name}</span>
                <span className="text-sm text-muted-foreground">
                  {name(issue.source)} ·{' '}
                  {issue.last_error
                    ? lastError(issue.last_error, t)
                    : issue.candidates > 0
                      ? t('admin.candidates', { count: issue.candidates })
                      : t('admin.notFoundAnywhere')}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
