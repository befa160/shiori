import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ChevronRight, Loader2 } from 'lucide-react'
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useCurrentScan, useLibraries } from '@/api/queries'
import type { LogLine, RunCounts, RunStatus, RunSummary, Severity } from '@/api/types'
import { BackLink, QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Progress } from '@/components/ui/progress'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { errorText, issueText } from '@/i18n/errors'
import { cn } from '@/lib/utils'
import { formatDate, formatDuration } from './format'
import { useStartScan } from './useStartScan'

const STATUSES: RunStatus[] = ['running', 'completed', 'cancelled', 'failed', 'interrupted']
const PAGE = 20
const LOG_PAGE = 100
const ALL = 'all'

/** Monté dans le cadre commun : à la fin d'un scan, les listes sont redemandées et une
 * notification dit comment il s'est terminé, quel que soit l'écran affiché. */
export function ScanWatcher() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const { data, dataUpdatedAt } = useCurrentScan()
  const running = useRef<number | null>(null)
  // À chaque réponse du serveur (et pas seulement quand elle change : « rien », puis un scan
  // lancé et déjà fini, puis « rien » ne change pas la valeur).
  useEffect(() => {
    if (data === undefined) return
    const started = queryClient.getQueryData<number>(['scans', 'started'])
    queryClient.removeQueries({ queryKey: ['scans', 'started'], exact: true })
    if (data !== null) {
      running.current = data.run
      return
    }
    const run = running.current ?? started ?? null
    if (run === null) return
    running.current = null
    for (const queryKey of [['libraries'], ['series'], ['continue'], ['volumes'], ['scans', 'history'], ['scans', 'run', run], ['scans', 'logs', run]]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    api
      .scan(run)
      .then((summary) => toast(t(`scans.ended.${summary.status}`), { description: countsText(summary.counts, t) }))
      .catch(() => toast(t('scans.ended.completed')))
  }, [data, dataUpdatedAt, queryClient, t])
  return null
}

type T = ReturnType<typeof useTranslation>['t']
const countsText = (c: RunCounts, t: T) =>
  (['ok', 'warning', 'error'] as const).map((k) => t('scans.countItem', { label: t(`scans.counts.${k}`), value: c[k] })).join(' · ')

function Counts({ counts }: { counts: RunCounts }) {
  const { t, i18n } = useTranslation()
  const n = new Intl.NumberFormat(i18n.language)
  return (
    <dl className="flex flex-wrap gap-x-4 gap-y-1 text-sm">
      {(['ok', 'warning', 'error', 'unchanged', 'removed'] as const).map((k) => (
        <div key={k} className="flex gap-1">
          <dt className="text-muted-foreground">{t(`scans.counts.${k}`)}</dt>
          <dd className={cn('tabular-nums', k === 'error' && counts.error > 0 && 'text-destructive')}>{n.format(counts[k])}</dd>
        </div>
      ))}
    </dl>
  )
}

/** Passe en cours : progression, fichier courant, compteurs, annulation. */
export function CurrentScan() {
  const { t, i18n } = useTranslation()
  // Toutes les 10 s au repos tant que la carte est affichée : un scan lancé ailleurs apparaît.
  const current = useCurrentScan(10_000)
  const libraries = useLibraries().data
  const queryClient = useQueryClient()
  const cancel = useMutation({
    mutationFn: api.cancelScan,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['scans', 'current'] }),
    onError: (error) => toast.error(t('scans.cancelNotSent'), { description: errorText(error, t) }),
  })
  return (
    <QueryState query={current} isEmpty={(p) => p === null} empty={<p>{t('scans.idle')}</p>}>
      {(p) => {
        if (p === null) return null
        const name = libraries?.find((l) => l.id === p.library_id)?.name ?? t('library.fallbackName')
        const n = new Intl.NumberFormat(i18n.language)
        return (
          <div className="space-y-3 rounded-md bg-card p-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h3 title={name} className="min-w-0 truncate font-medium">
                {t('scans.currentNamed', { name })}
              </h3>
              <Button variant="secondary" disabled={p.cancelling || cancel.isPending} onClick={() => cancel.mutate()}>
                {(p.cancelling || cancel.isPending) && <Loader2 aria-hidden className="animate-spin" />}
                {t(p.cancelling ? 'scans.cancelling' : 'scans.cancel')}
              </Button>
            </div>
            <Progress aria-label={t('scans.progressLabel')} value={p.total ? (p.done / p.total) * 100 : 0} className="h-2" />
            <p className="text-sm">
              {p.total ? t('scans.progress', { done: n.format(p.done), total: n.format(p.total) }) : t('scans.preparing')}
            </p>
            <Counts counts={p.counts} />
            {p.current && (
              <p title={p.current} className="truncate font-mono text-xs text-muted-foreground">
                {p.current}
              </p>
            )}
          </div>
        )
      }}
    </QueryState>
  )
}

/** Lancer un scan avec ses options, chacune expliquée. */
function LaunchScan() {
  const { t } = useTranslation()
  const id = useId()
  const libraries = useLibraries().data ?? []
  const running = Boolean(useCurrentScan().data)
  const scan = useStartScan()
  const [library, setLibrary] = useState<string>('')
  const [deep, setDeep] = useState(false)
  const [rehash, setRehash] = useState(false)
  const chosen = libraries.find((l) => String(l.id) === library) ?? libraries[0]
  if (!chosen) return null
  const submit = (e: FormEvent) => {
    e.preventDefault()
    scan.mutate({ library: chosen, options: { deep, rehash } })
  }
  const option = (key: 'deep' | 'rehash', checked: boolean, set: (v: boolean) => void) => (
    <div className="flex items-start gap-3">
      <Checkbox id={`${id}-${key}`} checked={checked} onCheckedChange={(v) => set(v === true)} aria-describedby={`${id}-${key}-help`} className="mt-3" />
      <div>
        <label htmlFor={`${id}-${key}`} className="flex min-h-11 items-center text-sm font-medium">
          {t(`scans.${key}`)}
        </label>
        <p id={`${id}-${key}-help`} className="-mt-2 text-sm text-muted-foreground">
          {t(`scans.${key}Help`)}
        </p>
      </div>
    </div>
  )
  return (
    <form onSubmit={submit} aria-labelledby={`${id}-title`} className="space-y-3 rounded-md bg-card p-4">
      <h3 id={`${id}-title`} className="font-medium">
        {t('scans.launch')}
      </h3>
      <div className="space-y-1">
        <p id={`${id}-library`} className="text-sm text-muted-foreground">
          {t('scans.library')}
        </p>
        <Select value={String(chosen.id)} onValueChange={setLibrary}>
          <SelectTrigger aria-labelledby={`${id}-library`} className="w-full max-w-sm *:data-[slot=select-value]:block *:data-[slot=select-value]:truncate">
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper" align="start">
            {libraries.map((l) => (
              <SelectItem key={l.id} value={String(l.id)}>
                {l.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {option('deep', deep, setDeep)}
      {option('rehash', rehash, setRehash)}
      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" disabled={running || scan.isPending}>
          {scan.isPending && <Loader2 aria-hidden className="animate-spin" />}
          {t('admin.scan')}
        </Button>
        {running && <p className="text-sm text-muted-foreground">{t('scans.busy')}</p>}
      </div>
    </form>
  )
}

function StatusText({ status }: { status: RunStatus }) {
  const { t } = useTranslation()
  return <span className={cn('font-medium', status === 'failed' && 'text-destructive', status === 'running' && 'text-primary')}>{t(`scans.status.${status}`)}</span>
}

/** Historique filtrable (statut, bibliothèque), du plus récent au plus ancien, par pages. */
function History() {
  const { t, i18n } = useTranslation()
  const [params, setParams] = useSearchParams()
  const libraries = useLibraries().data ?? []
  const status = params.get('status') ?? ALL
  const library = params.get('library') ?? ALL
  const history = useInfiniteQuery({
    queryKey: ['scans', 'history', status, library],
    queryFn: ({ pageParam, signal }) => {
      const q = new URLSearchParams({ limit: String(PAGE) })
      if (status !== ALL) q.set('status', status)
      if (library !== ALL) q.set('library_id', library)
      if (pageParam !== null) q.set('before', String(pageParam))
      return api.scans(q, { signal })
    },
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length === PAGE ? last[last.length - 1].id : null),
  })
  const filter = (key: string, value: string) =>
    setParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (value === ALL) next.delete(key)
        else next.set(key, value)
        return next
      },
      { replace: true },
    )
  const libraryName = (run: RunSummary) => libraries.find((l) => l.id === run.library_id)?.name ?? run.root
  return (
    <section aria-labelledby="scans-history" className="space-y-3">
      <h2 id="scans-history" className="text-lg font-semibold">
        {t('scans.history')}
      </h2>
      <div className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap">
        <Select value={status} onValueChange={(v) => filter('status', v)}>
          <SelectTrigger aria-label={t('scans.statusLabel')} className={cn('w-full sm:w-fit', status !== ALL && 'border-primary text-primary')}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper" align="start">
            <SelectItem value={ALL}>{t('scans.statusAll')}</SelectItem>
            {STATUSES.map((s) => (
              <SelectItem key={s} value={s}>
                {t(`scans.status.${s}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {/* Aussi quand un filtre est actif : une bibliothèque supprimée ne doit pas le rendre impossible à retirer. */}
        {(libraries.length > 1 || library !== ALL) && (
          <Select value={library} onValueChange={(v) => filter('library', v)}>
            <SelectTrigger
              aria-label={t('browse.libraryLabel')}
              className={cn('w-full min-w-0 sm:w-fit *:data-[slot=select-value]:block *:data-[slot=select-value]:truncate', library !== ALL && 'border-primary text-primary')}
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              <SelectItem value={ALL}>{t('browse.libraryAll')}</SelectItem>
              {libraries.map((l) => (
                <SelectItem key={l.id} value={String(l.id)}>
                  {l.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </div>
      <QueryState query={history} isEmpty={(d) => d.pages[0].length === 0} empty={<p>{t('scans.empty')}</p>}>
        {(d) => (
          <>
            <ul className="space-y-2">
              {d.pages.flat().map((run) => (
                <li key={run.id}>
                  <Link
                    to={`/admin/scans/${run.id}`}
                    className="flex min-h-14 items-center gap-3 rounded-md bg-card px-4 py-3 transition-colors hover:bg-accent active:bg-muted"
                  >
                    <div className="min-w-0 flex-1 space-y-1">
                      <p className="flex flex-wrap gap-x-2 text-sm">
                        <StatusText status={run.status} />
                        <span className="text-muted-foreground">{formatDate(run.started_at, i18n.language)}</span>
                        {run.duration_ms !== null && <span className="text-muted-foreground">· {formatDuration(run.duration_ms, i18n.language)}</span>}
                      </p>
                      <p title={libraryName(run)} className="truncate">
                        {libraryName(run)}
                      </p>
                      <p className="text-sm text-muted-foreground">{countsText(run.counts, t)}</p>
                    </div>
                    <ChevronRight aria-hidden className="size-5 shrink-0 text-muted-foreground" />
                  </Link>
                </li>
              ))}
            </ul>
            {history.hasNextPage && (
              <Button variant="secondary" disabled={history.isFetchingNextPage} onClick={() => history.fetchNextPage()}>
                {history.isFetchingNextPage && <Loader2 aria-hidden className="animate-spin" />}
                {t('scans.more')}
              </Button>
            )}
          </>
        )}
      </QueryState>
    </section>
  )
}

/** `/admin/scans` : passe en cours, lancement avec options, historique. */
export function ScansPage() {
  const { t } = useTranslation()
  return (
    <section className="space-y-8">
      <title>{`${t('scans.title')} · Shiori`}</title>
      <div className="space-y-4">
        <BackLink to="/admin" label={t('admin.title')} />
        <h1 className="text-2xl font-semibold">{t('scans.title')}</h1>
      </div>
      <section aria-labelledby="scans-current" className="space-y-3">
        <h2 id="scans-current" className="text-lg font-semibold">
          {t('scans.current')}
        </h2>
        <CurrentScan />
        <LaunchScan />
      </section>
      <History />
    </section>
  )
}

const LEVELS: Severity[] = ['info', 'warning', 'error']

/** `/admin/scans/:id` : options, durée, compteurs, erreur fatale, puis le journal. */
export function ScanPage({ id }: { id: number }) {
  const { t, i18n } = useTranslation()
  const [params, setParams] = useSearchParams()
  const level = LEVELS.find((l) => l === params.get('level')) ?? 'info'
  const libraries = useLibraries().data
  const run = useQuery({
    queryKey: ['scans', 'run', id],
    queryFn: ({ signal }) => api.scan(id, { signal }),
    refetchInterval: (query) => (query.state.data?.status === 'running' ? 2000 : false),
  })
  const logs = useInfiniteQuery({
    queryKey: ['scans', 'logs', id, level],
    queryFn: ({ pageParam, signal }) => {
      const q = new URLSearchParams({ level, limit: String(LOG_PAGE) })
      if (pageParam !== null) q.set('after', String(pageParam))
      return api.scanLogs(id, q, { signal })
    },
    initialPageParam: null as number | null,
    getNextPageParam: (last) => (last.length === LOG_PAGE ? last[last.length - 1].id : null),
  })
  const title = t('scans.runTitle', { id })
  return (
    <section className="space-y-6">
      <title>{`${title} · Shiori`}</title>
      <BackLink to="/admin/scans" label={t('scans.title')} />
      <h1 className="text-2xl font-semibold">{title}</h1>
      <QueryState query={run} isEmpty={() => false} empty={null} notFound={t('scans.notFound')}>
        {(r) => {
          const yes = (v: boolean | undefined) => t(v ? 'scans.yes' : 'scans.no')
          const rows: [string, string][] = [
            [t('scans.library'), libraries?.find((l) => l.id === r.library_id)?.name ?? '—'],
            [t('scans.target'), r.root],
            [t('scans.origin'), t(`scans.origins.${r.origin}`)],
            [t('scans.started'), formatDate(r.started_at, i18n.language)],
            [t('scans.duration'), r.duration_ms === null ? '—' : formatDuration(r.duration_ms, i18n.language)],
            [t('scans.files'), r.files_total === null ? '—' : new Intl.NumberFormat(i18n.language).format(r.files_total)],
            [t('scans.deep'), yes(r.options.deep)],
            [t('scans.rehash'), yes(r.options.rehash)],
          ]
          return (
            <div className="space-y-4 rounded-md bg-card p-4">
              <p>
                <StatusText status={r.status} />
              </p>
              {r.error && (
                <p role="alert" className="text-destructive">
                  {t('scans.fatal', { error: r.error })}
                </p>
              )}
              <dl className="grid gap-x-6 gap-y-2 text-sm sm:grid-cols-[max-content_1fr]">
                {rows.map(([label, value]) => (
                  <div key={label} className="contents">
                    <dt className="text-muted-foreground">{label}</dt>
                    <dd className="[overflow-wrap:anywhere]">{value}</dd>
                  </div>
                ))}
              </dl>
              <Counts counts={r.counts} />
            </div>
          )
        }}
      </QueryState>
      <section aria-labelledby="scan-logs" className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 id="scan-logs" className="text-lg font-semibold">
            {t('scans.logs')}
          </h2>
          <Select
            value={level}
            onValueChange={(v) => setParams(v === 'info' ? {} : { level: v }, { replace: true })}
          >
            <SelectTrigger aria-label={t('scans.levelLabel')} className={cn(level !== 'info' && 'border-primary text-primary')}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="end">
              {LEVELS.map((l) => (
                <SelectItem key={l} value={l}>
                  {t(`scans.levels.${l}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <QueryState query={logs} isEmpty={(d) => d.pages[0].length === 0} empty={<p>{t('scans.noLogs')}</p>}>
          {(d) => (
            <>
              <ul className="space-y-2">
                {d.pages.flat().map((line) => (
                  <LogRow key={line.id} line={line} />
                ))}
              </ul>
              {logs.hasNextPage && (
                <Button variant="secondary" disabled={logs.isFetchingNextPage} onClick={() => logs.fetchNextPage()}>
                  {logs.isFetchingNextPage && <Loader2 aria-hidden className="animate-spin" />}
                  {t('scans.more')}
                </Button>
              )}
            </>
          )}
        </QueryState>
      </section>
    </section>
  )
}

function LogRow({ line }: { line: LogLine }) {
  const { t, i18n } = useTranslation()
  const time = new Intl.DateTimeFormat(i18n.language, { timeStyle: 'medium' }).format(new Date(line.at))
  // Un code connu se traduit ; sans code, le message du serveur (en français, Q21).
  const text = line.code ? issueText({ code: line.code, message: line.message }, t) : line.message
  return (
    <li className="space-y-1 rounded-md bg-card p-3 text-sm">
      <p>
        <span className="text-muted-foreground tabular-nums">{time}</span> ·{' '}
        <span className={cn('font-medium', line.level === 'error' && 'text-destructive')}>{t(`severity.${line.level}`)}</span> · {text}
      </p>
      {line.path && <p className="font-mono text-xs [overflow-wrap:anywhere] text-muted-foreground">{line.path}</p>}
      {line.series_id !== null && line.volume_id !== null && (
        <Link
          to={`/series/${line.series_id}#volume-${line.volume_id}`}
          className="inline-flex min-h-11 items-center gap-1 text-primary underline-offset-4 hover:underline"
        >
          {t('scans.openVolume')}
          <ChevronRight aria-hidden className="size-4" />
        </Link>
      )}
    </li>
  )
}
