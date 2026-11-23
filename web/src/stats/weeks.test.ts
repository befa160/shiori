import { describe, expect, it } from 'vitest'
import { byWeek, formatReadingTime, isoDay, periodDays, weekStarts } from './weeks'

const plain = (s: string) => s.replace(/\s/g, ' ')

describe('semaines des statistiques', () => {
  it('va du lundi d’il y a 11 semaines à celui de la semaine en cours, et demande ces jours-là', () => {
    const wednesday = new Date(2026, 10, 25)
    const starts = weekStarts(wednesday).map(isoDay)
    expect(starts).toHaveLength(12)
    expect(starts.at(-1)).toBe('2026-11-23')
    expect(starts[0]).toBe('2026-09-07')
    // Du 7 septembre au 25 novembre compris.
    expect(periodDays(wednesday)).toBe(80)
    // Le dimanche appartient encore à la semaine commencée le lundi d’avant.
    expect(isoDay(weekStarts(new Date(2026, 10, 29)).at(-1)!)).toBe('2026-11-23')
    expect(periodDays(new Date(2026, 10, 23))).toBe(78)
  })

  it('additionne les jours de chaque semaine ; une semaine sans lecture vaut zéro', () => {
    const today = new Date(2026, 10, 25)
    const weeks = byWeek(
      [
        { day: '2026-09-01', pages: 99, seconds: 99, finished: 9 }, // avant la période
        { day: '2026-09-07', pages: 10, seconds: 60, finished: 0 },
        { day: '2026-11-22', pages: 5, seconds: 30, finished: 1 },
        { day: '2026-11-23', pages: 20, seconds: 600, finished: 1 },
        { day: '2026-11-25', pages: 2, seconds: 0, finished: 0 },
      ],
      today,
    )
    expect(weeks.map((w) => w.pages)).toEqual([10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 22])
    expect(weeks.at(-1)).toMatchObject({ seconds: 600, finished: 1 })
  })

  it('écrit le temps en minutes, puis en heures et minutes', () => {
    expect(plain(formatReadingTime(0, 'fr'))).toBe('0 min')
    expect(plain(formatReadingTime(2700, 'fr'))).toBe('45 min')
    expect(plain(formatReadingTime(3600, 'fr'))).toBe('1 h')
    expect(plain(formatReadingTime(11_100, 'fr'))).toBe('3 h 5 min')
  })
})
