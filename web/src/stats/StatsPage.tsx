import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api } from '@/api/client'
import type { ReadingStats } from '@/api/types'
import { QueryState } from '@/browse/Parts'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { byWeek, formatReadingTime, periodDays, type Week } from './weeks'

/** Statistiques de lecture du compte (lot 1.1b) : totaux, puis les 12 dernières semaines en
 * colonnes, avec leur tableau (les valeurs ne dépendent jamais du survol). */
export function StatsPage() {
  const { t } = useTranslation()
  // Fixé à l'ouverture : la période demandée et les semaines affichées restent d'accord.
  const [today] = useState(() => new Date())
  const days = periodDays(today)
  const query = useQuery({ queryKey: ['stats', days], queryFn: ({ signal }) => api.readingStats(days, { signal }) })
  return (
    <section className="max-w-4xl space-y-6">
      <title>{`${t('stats.title')} · Shiori`}</title>
      <div className="space-y-1">
        <h1 className="text-2xl font-semibold">{t('stats.title')}</h1>
        <p className="text-sm text-muted-foreground">{t('stats.help')}</p>
      </div>
      <QueryState
        query={query}
        isEmpty={(s) => s.total.pages === 0 && s.total.finished === 0 && s.series_finished === 0}
        empty={
          <>
            <p className="font-medium text-foreground">{t('stats.empty')}</p>
            <p className="text-sm">{t('stats.emptyHelp')}</p>
          </>
        }
      >
        {(stats) => <Stats stats={stats} weeks={byWeek(stats.days, today)} />}
      </QueryState>
    </section>
  )
}

function Stats({ stats, weeks }: { stats: ReadingStats; weeks: Week[] }) {
  const { t, i18n } = useTranslation()
  const number = (n: number) => new Intl.NumberFormat(i18n.language).format(n)
  const time = (seconds: number) => formatReadingTime(seconds, i18n.language)
  const period = weeks.reduce((sum, w) => ({ pages: sum.pages + w.pages, seconds: sum.seconds + w.seconds, finished: sum.finished + w.finished }), {
    pages: 0,
    seconds: 0,
    finished: 0,
  })
  const tiles = [
    { label: 'stats.pages', value: number(stats.total.pages) },
    { label: 'stats.time', value: time(stats.total.seconds) },
    { label: 'stats.finished', value: number(stats.total.finished) },
    { label: 'stats.seriesFinished', value: number(stats.series_finished) },
  ] as const
  return (
    <>
      <dl className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {tiles.map(({ label, value }) => (
          <div key={label} className="space-y-1 rounded-md bg-card p-4">
            <dt className="text-sm text-muted-foreground">{t(label)}</dt>
            <dd className="text-2xl font-semibold">{value}</dd>
          </div>
        ))}
      </dl>
      <section aria-labelledby="stats-weeks" className="space-y-3 rounded-md bg-card p-4">
        <div className="space-y-1">
          <h2 id="stats-weeks" className="text-lg font-semibold">
            {t('stats.weeks')}
          </h2>
          <p className="text-sm text-muted-foreground">
            {t('stats.summary', { pages: number(period.pages), time: time(period.seconds), finished: number(period.finished) })}
          </p>
        </div>
        <WeekChart weeks={weeks} />
        <details className="text-sm">
          <summary className="cursor-pointer py-2 font-medium">{t('stats.details')}</summary>
          <div className="overflow-x-auto">
            <table className="w-full text-left tabular-nums">
              <thead className="text-muted-foreground">
                <tr>
                  <th scope="col" className="py-2 pr-4 font-normal">
                    {t('stats.weekColumn')}
                  </th>
                  <th scope="col" className="py-2 pr-4 text-right font-normal">
                    {t('stats.pages')}
                  </th>
                  <th scope="col" className="py-2 pr-4 text-right font-normal">
                    {t('stats.time')}
                  </th>
                  <th scope="col" className="py-2 text-right font-normal">
                    {t('stats.finished')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {weeks.toReversed().map((w) => (
                  <tr key={w.start.getTime()} className="border-t border-border">
                    <th scope="row" className="py-2 pr-4 font-normal whitespace-nowrap">
                      {shortDate(w.start, i18n.language)}
                    </th>
                    <td className="py-2 pr-4 text-right">{number(w.pages)}</td>
                    <td className="py-2 pr-4 text-right whitespace-nowrap">{time(w.seconds)}</td>
                    <td className="py-2 text-right">{number(w.finished)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </details>
      </section>
    </>
  )
}

const shortDate = (date: Date, language: string) => new Intl.DateTimeFormat(language, { day: 'numeric', month: 'short' }).format(date)

/** Pages par semaine en colonnes (DESIGN_SYSTEM : l'ambre est celui des barres de progression).
 * Masqué aux lecteurs d'écran : le résumé et le tableau donnent les mêmes valeurs. */
function WeekChart({ weeks }: { weeks: Week[] }) {
  const { t, i18n } = useTranslation()
  const number = (n: number) => new Intl.NumberFormat(i18n.language).format(n)
  const max = Math.max(...weeks.map((w) => w.pages))
  const peak = weeks.findLastIndex((w) => w.pages === max && max > 0)
  // Quatre dates sous l'axe : elles tiennent côte à côte même sur un téléphone.
  const labelled = new Set([0, 4, 8, weeks.length - 1])
  return (
    <TooltipProvider delayDuration={0}>
      <div aria-hidden className="space-y-1">
        <div className="flex h-40 items-end gap-0.5 border-b border-border pt-5">
          {weeks.map((w, i) => (
            <div key={w.start.getTime()} className="flex h-full flex-1 items-end justify-center">
              {w.pages > 0 && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div
                      data-testid="week-bar"
                      className="relative w-full max-w-6 rounded-t-[4px] bg-primary transition-opacity hover:opacity-80"
                      style={{ height: `${(w.pages / max) * 100}%` }}
                    >
                      {i === peak && (
                        <span className="absolute -top-5 left-1/2 -translate-x-1/2 text-xs whitespace-nowrap text-muted-foreground">
                          {number(w.pages)}
                        </span>
                      )}
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p className="font-semibold">{t('stats.summary', { pages: number(w.pages), time: formatReadingTime(w.seconds, i18n.language), finished: number(w.finished) })}</p>
                    <p>{t('stats.week', { date: shortDate(w.start, i18n.language) })}</p>
                  </TooltipContent>
                </Tooltip>
              )}
            </div>
          ))}
        </div>
        <div className="flex gap-0.5 text-xs text-muted-foreground">
          {weeks.map((w, i) => (
            <span key={w.start.getTime()} className="flex-1 text-center whitespace-nowrap">
              {labelled.has(i) ? shortDate(w.start, i18n.language) : ''}
            </span>
          ))}
        </div>
      </div>
    </TooltipProvider>
  )
}
