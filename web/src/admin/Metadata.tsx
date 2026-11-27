import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowDown, ArrowUp, Loader2 } from 'lucide-react'
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { InstancePatch, RefreshDays, SourceId, SourceIssue } from '@/api/types'
import { QueryState } from '@/browse/Parts'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Progress } from '@/components/ui/progress'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { sourceErrorText, toastFailure } from '@/i18n/errors'

/** « Actualiser les fiches liées » (1.4f) : jamais (par défaut, Q80), chaque semaine, chaque mois. */
const REFRESH_DAYS: RefreshDays[] = [0, 7, 30]

const CREDITS: [SourceId, string][] = [
  ['anilist', 'https://anilist.co'],
  ['mangadex', 'https://mangadex.org'],
  ['wikipedia', 'https://www.wikipedia.org'],
  ['mangaupdates', 'https://www.mangaupdates.com'],
  ['comicvine', 'https://comicvine.gamespot.com'],
]

/** Les sources créditées, comme leurs conditions le demandent ; Wikipédia avec la licence de ses
 * textes (CC BY-SA), le lien vers l'article étant dans les liens de la série. */
export function Credits() {
  const { t } = useTranslation()
  return (
    <p className="text-sm text-muted-foreground">
      {t('admin.credits')}{' '}
      {CREDITS.map(([source, href], i) => (
        <span key={source}>
          {i > 0 && ', '}
          <a href={href} target="_blank" rel="noreferrer noopener" className="underline underline-offset-4">
            {t(`admin.sourceNames.${source}`)}
          </a>
          {source === 'wikipedia' && ` (${t('admin.license')})`}
        </span>
      ))}
      .
    </p>
  )
}

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
    mutationFn: (patch: InstancePatch) => api.saveSettings(patch),
    onSuccess: (saved) => {
      queryClient.setQueryData(['settings'], saved)
      // L'ordre change les titres et résumés venus du web.
      for (const queryKey of [['admin', 'metadata'], ['series'], ['libraries'], ['continue']]) {
        void queryClient.invalidateQueries({ queryKey })
      }
    },
    onError: toastFailure(t('admin.metadataNotSaved'), t),
  })
  const identify = useMutation({
    mutationFn: api.identify,
    onSuccess: () => {
      toast.success(t('admin.identifyStarted'))
      void queryClient.invalidateQueries({ queryKey: ['admin', 'metadata'] })
    },
    onError: toastFailure(t('admin.identifyNotStarted'), t),
  })
  // Valeur en cours d'enregistrement, sinon celle du serveur.
  const pending = <K extends 'metadata_online' | 'metadata_adult'>(key: K) =>
    save.isPending && save.variables[key] !== undefined ? save.variables[key] : settings.data?.[key]
  const online = pending('metadata_online')
  const adult = pending('metadata_adult')
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
      {adult !== undefined && (
        <div className="flex items-start gap-3">
          <Checkbox
            id={`${id}-adult`}
            checked={adult}
            onCheckedChange={(v) => save.mutate({ metadata_adult: v === true })}
            aria-describedby={`${id}-adult-help`}
            className="mt-3"
          />
          <div>
            <label htmlFor={`${id}-adult`} className="flex min-h-11 items-center text-sm font-medium">
              {t('admin.metadataAdult')}
            </label>
            <p id={`${id}-adult-help`} className="-mt-2 text-sm text-muted-foreground">
              {t('admin.metadataAdultHelp')}
            </p>
          </div>
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
      {settings.data && (
        <div className="space-y-1">
          <p id={`${id}-refresh`} className="text-sm font-medium">
            {t('admin.refresh')}
          </p>
          <p id={`${id}-refresh-help`} className="text-sm text-muted-foreground">
            {t('admin.refreshHelp')}
          </p>
          <Select
            value={String(save.isPending && save.variables.metadata_refresh_days !== undefined ? save.variables.metadata_refresh_days : settings.data.metadata_refresh_days)}
            onValueChange={(v) => save.mutate({ metadata_refresh_days: Number(v) as RefreshDays })}
          >
            <SelectTrigger aria-labelledby={`${id}-refresh`} aria-describedby={`${id}-refresh-help`} className="w-full max-w-sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              {REFRESH_DAYS.map((days) => (
                <SelectItem key={days} value={String(days)}>
                  {t(`admin.refreshEvery.d${days}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      {settings.data && (
        <ComicvineKey set={settings.data.comicvine_key_set} saving={save.isPending} onSave={(comicvine_key) => save.mutate({ comicvine_key })} />
      )}
      <Credits />
    </section>
  )
}

/** Clé Comic Vine (Q79) : saisie ici, gardée sur le serveur, jamais réaffichée. */
function ComicvineKey({ set, saving, onSave }: { set: boolean; saving: boolean; onSave: (key: string | null) => void }) {
  const { t } = useTranslation()
  const id = useId()
  const [key, setKey] = useState('')
  const submit = (e: FormEvent) => {
    e.preventDefault()
    onSave(key.trim())
    setKey('')
  }
  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium">
        <label htmlFor={`${id}-key`}>{t('admin.comicvine')}</label>
      </h3>
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {t('admin.comicvineHelp')}{' '}
        <a href="https://comicvine.gamespot.com/api/" target="_blank" rel="noreferrer noopener" className="underline underline-offset-4">
          {t('admin.comicvineGet')}
        </a>
      </p>
      {set ? (
        <div className="flex flex-wrap items-center gap-3">
          <p className="text-sm">{t('admin.comicvineSet')}</p>
          <Button variant="secondary" disabled={saving} onClick={() => onSave(null)}>
            {t('admin.comicvineClear')}
          </Button>
        </div>
      ) : (
        <form onSubmit={submit} className="flex flex-wrap gap-2">
          <Input
            id={`${id}-key`}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            className="w-72 max-w-full font-mono"
            aria-describedby={`${id}-help`}
          />
          <Button type="submit" variant="secondary" disabled={saving || key.trim() === ''}>
            {t('admin.comicvineSave')}
          </Button>
        </form>
      )}
    </div>
  )
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
                    ? sourceErrorText(issue.last_error, t)
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
