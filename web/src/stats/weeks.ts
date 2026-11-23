import type { Activity, DayActivity } from '@/api/types'

/** Semaines affichées : la semaine en cours et les 11 d'avant (12 semaines, Q60). */
export const WEEKS = 12

export interface Week extends Activity {
  /** Lundi de la semaine, minuit, heure de l'appareil. */
  start: Date
}

/** `AAAA-MM-JJ` d'une date, dans le fuseau de l'appareil (celui des jours renvoyés par l'API). */
export function isoDay(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** Lundis des semaines affichées, du plus ancien à celui de la semaine de `today`. */
export function weekStarts(today: Date): Date[] {
  const sinceMonday = (today.getDay() + 6) % 7
  // Arithmétique sur les jours du calendrier, pas sur les millisecondes : juste aux changements d'heure.
  return Array.from(
    { length: WEEKS },
    (_, i) => new Date(today.getFullYear(), today.getMonth(), today.getDate() - sinceMonday - 7 * (WEEKS - 1 - i)),
  )
}

/** Jours à demander à l'API pour couvrir toutes les semaines affichées, aujourd'hui compris. */
export function periodDays(today: Date): number {
  return 7 * (WEEKS - 1) + ((today.getDay() + 6) % 7) + 1
}

/** Jours lus regroupés par semaine ; une semaine sans lecture vaut zéro. */
export function byWeek(days: DayActivity[], today: Date): Week[] {
  const weeks = weekStarts(today).map((start) => ({ start, pages: 0, seconds: 0, finished: 0 }))
  const starts = weeks.map((w) => isoDay(w.start))
  for (const day of days) {
    const i = starts.findLastIndex((start) => start <= day.day)
    if (i < 0) continue
    weeks[i].pages += day.pages
    weeks[i].seconds += day.seconds
    weeks[i].finished += day.finished
  }
  return weeks
}

/** Durée de lecture lisible : « 45 min », « 3 h 5 min ». */
export function formatReadingTime(seconds: number, language: string): string {
  const unit = (value: number, u: 'hour' | 'minute') =>
    new Intl.NumberFormat(language, { style: 'unit', unit: u, unitDisplay: 'short' }).format(value)
  const minutes = Math.round(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours === 0) return unit(minutes, 'minute')
  return minutes % 60 === 0 ? unit(hours, 'hour') : `${unit(hours, 'hour')} ${unit(minutes % 60, 'minute')}`
}
