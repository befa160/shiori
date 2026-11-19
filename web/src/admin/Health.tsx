import { useInfiniteQuery, useQuery } from '@tanstack/react-query'
import { BookOpen, Loader2, ShieldCheck } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import { api } from '@/api/client'
import type { FileHealth, LibraryHealth } from '@/api/types'
import { BackLink, QueryState } from '@/browse/parts'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { issueText } from '@/i18n/errors'
import { docsUrl } from '@/lib/docs'
import { cn } from '@/lib/utils'
import { formatDate } from './format'
import { useStartScan } from './useStartScan'

const PAGE = 50
const ALL = 'all'

/** `/admin/health` : état de chaque bibliothèque, vérification en profondeur, fichiers à réparer. */
export function HealthPage() {
  const { t } = useTranslation()
  const health = useQuery({ queryKey: ['health'], queryFn: ({ signal }) => api.librariesHealth({ signal }) })
  return (
    <section className="space-y-8">
      <title>{`${t('health.title')} · Shiori`}</title>
      <div className="space-y-4">
        <BackLink to="/admin" label={t('admin.title')} />
        <h1 className="text-2xl font-semibold">{t('health.title')}</h1>
      </div>
      <QueryState query={health} isEmpty={(h) => h.libraries.length === 0} empty={<p>{t('admin.empty')}</p>}>
        {(h) => (
          <>
            <ul className="grid gap-3 lg:grid-cols-2">
              {h.libraries.map((library) => (
                <LibraryCard key={library.id} library={library} />
              ))}
            </ul>
            <ToRepair libraries={h.libraries} codes={h.codes.map((c) => c.code)} />
          </>
        )}
      </QueryState>
    </section>
  )
}

function LibraryCard({ library }: { library: LibraryHealth }) {
  const { t, i18n } = useTranslation()
  const scan = useStartScan()
  const n = new Intl.NumberFormat(i18n.language)
  // Libellé à côté de chaque nombre : l'état ne passe jamais par la couleur seule.
  const counts = [
    ['ok', library.ok, ''],
    ['warning', library.warning, ''],
    ['error', library.error, 'text-destructive'],
    ['unreadable', library.unreadable, 'text-destructive'],
  ] as const
  return (
    <li className="flex flex-col gap-3 rounded-md bg-card p-4">
      <h2 title={library.name} className="truncate text-lg font-semibold">
        {library.name}
      </h2>
      <dl className="flex flex-wrap gap-x-4 gap-y-1 text-sm">
        {counts.map(([key, value, tone]) => (
          <div key={key} className="flex gap-1">
            <dt className="text-muted-foreground">{t(`health.counts.${key}`)}</dt>
            <dd className={cn('tabular-nums', value > 0 && tone)}>{n.format(value)}</dd>
          </div>
        ))}
      </dl>
      <p className="text-sm text-muted-foreground">
        {library.deep_checked_at ? t('health.deepCheckedAt', { date: formatDate(library.deep_checked_at, i18n.language) }) : t('health.neverDeepChecked')}
      </p>
      <Button
        variant="secondary"
        className="self-start"
        disabled={scan.isPending}
        onClick={() => scan.mutate({ library, options: { deep: true } })}
        aria-label={t('health.deepCheckNamed', { name: library.name })}
      >
        {scan.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <ShieldCheck aria-hidden />}
        {t('health.deepCheck')}
      </Button>
    </li>
  )
}

/** Fichiers à réparer, filtrables par bibliothèque et par code (état dans l'adresse), par pages. */
function ToRepair({ libraries, codes }: { libraries: LibraryHealth[]; codes: string[] }) {
  const { t, i18n } = useTranslation()
  const [params, setParams] = useSearchParams()
  const library = params.get('library') ?? ALL
  const code = params.get('code') ?? ALL
  const files = useInfiniteQuery({
    queryKey: ['health', 'files', library, code],
    queryFn: ({ pageParam, signal }) => {
      const q = new URLSearchParams({ limit: String(PAGE) })
      if (library !== ALL) q.set('library_id', library)
      if (code !== ALL) q.set('code', code)
      if (pageParam !== null) q.set('after', pageParam)
      return api.filesToRepair(q, { signal })
    },
    initialPageParam: null as string | null,
    getNextPageParam: (last) => (last.length === PAGE ? last[last.length - 1].path : null),
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
  const label = (c: string) => issueText({ code: c, message: c }, t)
  return (
    <section aria-labelledby="health-repair" className="space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <h2 id="health-repair" className="text-lg font-semibold">
          {t('health.toRepair')}
        </h2>
        <a
          href={docsUrl(i18n.language, 'guide/diagnostics')}
          target="_blank"
          aria-label={`${t('health.codesHelp')} ${t('layout.newTab')}`}
          rel="noopener noreferrer"
          className="inline-flex min-h-11 items-center gap-1 text-sm text-primary underline-offset-4 hover:underline"
        >
          <BookOpen aria-hidden className="size-4" />
          {t('health.codesHelp')}
        </a>
      </div>
      <div className="grid grid-cols-1 gap-2 sm:flex sm:flex-wrap">
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
        {/* Aussi quand un filtre est actif : un code réparé entre-temps doit pouvoir être retiré. */}
        {(codes.length > 0 || code !== ALL) && (
          <Select value={code} onValueChange={(v) => filter('code', v)}>
            <SelectTrigger
              aria-label={t('health.codeLabel')}
              className={cn('w-full min-w-0 sm:w-fit sm:max-w-md *:data-[slot=select-value]:block *:data-[slot=select-value]:truncate', code !== ALL && 'border-primary text-primary')}
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              <SelectItem value={ALL}>{t('health.codeAll')}</SelectItem>
              {[...new Set([...codes, ...(code === ALL ? [] : [code])])].map((c) => (
                <SelectItem key={c} value={c}>
                  {label(c)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </div>
      <QueryState
        query={files}
        isEmpty={(d) => d.pages[0].length === 0}
        empty={
          <p className="flex items-center gap-2 rounded-md bg-card p-4">
            <ShieldCheck aria-hidden className="size-5 shrink-0 text-primary" />
            {t('health.nothing')}
          </p>
        }
      >
        {(d) => (
          <>
            <ul className="space-y-2">
              {d.pages.flat().map((file) => (
                <FileRow key={file.path} file={file} />
              ))}
            </ul>
            {files.hasNextPage && (
              <Button variant="secondary" disabled={files.isFetchingNextPage} onClick={() => files.fetchNextPage()}>
                {files.isFetchingNextPage && <Loader2 aria-hidden className="animate-spin" />}
                {t('scans.more')}
              </Button>
            )}
          </>
        )}
      </QueryState>
    </section>
  )
}

function FileRow({ file }: { file: FileHealth }) {
  const { t } = useTranslation()
  const name = file.path.slice(file.path.lastIndexOf('/') + 1)
  return (
    <li className="space-y-2 rounded-md bg-card p-4">
      <p className="flex flex-wrap items-baseline gap-x-2">
        <span className={cn('text-sm font-medium', file.status === 'error' && 'text-destructive')}>{t(`severity.${file.status}`)}</span>
        <span title={file.path} className="min-w-0 [overflow-wrap:anywhere]">
          {name}
        </span>
      </p>
      <p className="font-mono text-xs [overflow-wrap:anywhere] text-muted-foreground">{file.path}</p>
      <ul className="list-disc space-y-1 pl-5 text-sm">
        {file.issues.map((issue, i) => (
          <li key={i} className={cn(issue.severity === 'info' && 'text-muted-foreground')}>
            {issueText(issue, t)}
            {issue.entry_name && <span className="text-muted-foreground"> ({issue.entry_name})</span>}
          </li>
        ))}
      </ul>
      {file.series_id !== null && file.volume_id !== null && (
        <Button asChild variant="secondary">
          <Link to={`/series/${file.series_id}#volume-${file.volume_id}`}>{t('health.seeVolume')}</Link>
        </Button>
      )}
    </li>
  )
}
