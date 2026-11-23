import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ReadingStats } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'
import { periodDays } from './weeks'

const NOW = new Date(2026, 10, 25, 20, 0)
const url = `/api/me/stats?days=${periodDays(NOW)}&utc_offset=${-NOW.getTimezoneOffset()}`
const plain = (s: string | null) => s?.replace(/\s/g, ' ')

beforeEach(() => {
  vi.useFakeTimers({ now: NOW, toFake: ['Date'] })
})
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

const stats: ReadingStats = {
  days: [
    { day: '2026-11-10', pages: 40, seconds: 1800, finished: 0 },
    { day: '2026-11-24', pages: 1200, seconds: 7500, finished: 2 },
  ],
  total: { pages: 5210, seconds: 70_200, finished: 31 },
  series_finished: 4,
}

describe('page des statistiques', () => {
  it('montre les totaux, les 12 semaines en colonnes et leur tableau', async () => {
    mockApi({ '/api/libraries': () => json([]), [url]: () => json(stats) })
    renderAt('/stats')
    const main = within(screen.getByRole('main'))
    expect(main.getByRole('heading', { name: 'Statistiques', level: 1 })).toBeInTheDocument()
    // Le libellé revient en tête du tableau : la tuile est le premier.
    const tile = async (label: string) => plain((await main.findAllByText(label))[0].nextElementSibling?.textContent ?? null)
    expect(await tile('Pages lues')).toBe('5 210')
    expect(await tile('Temps de lecture')).toBe('19 h 30 min')
    expect(await tile('Tomes terminés')).toBe('31')
    expect(await tile('Séries terminées')).toBe('4')
    // Résumé de la période : ce qui est écrit, pas seulement dessiné.
    expect(plain(main.getByText(/^Pages : /).textContent)).toBe('Pages : 1 240 · Temps : 2 h 35 min · Tomes terminés : 2')
    // Deux semaines lues, deux colonnes ; la plus haute porte sa valeur.
    const bars = screen.getAllByTestId('week-bar')
    expect(bars).toHaveLength(2)
    expect(bars[1].style.height).toBe('100%')
    expect(plain(bars[1].textContent)).toBe('1 200')
    // Le tableau, de la semaine en cours à la plus ancienne.
    await userEvent.click(main.getByText('Détail par semaine'))
    const rows = within(main.getByRole('table')).getAllByRole('row')
    expect(rows).toHaveLength(13)
    expect(rows[1].textContent).toContain('23 nov.')
    expect(plain(rows[1].textContent)).toContain('1 200')
  })

  it('dit quand rien n’a encore été lu, et redemande après une erreur', async () => {
    let fail = true
    mockApi({
      '/api/libraries': () => json([]),
      [url]: () => (fail ? json({ code: 'internal', message: 'panne' }, 500) : json({ days: [], total: { pages: 0, seconds: 0, finished: 0 }, series_finished: 0 })),
    })
    renderAt('/stats')
    const main = within(screen.getByRole('main'))
    fail = false
    await userEvent.click(await main.findByRole('button', { name: 'Réessayer' }))
    expect(await main.findByText('Rien de lu pour l’instant')).toBeInTheDocument()
  })

  it('s’ouvre depuis la barre latérale et depuis la page du compte', async () => {
    mockApi({ '/api/libraries': () => json([]), '/api/me/sessions': () => json([]), [url]: () => json(stats) })
    const { router } = renderAt('/account')
    const main = within(screen.getByRole('main'))
    await userEvent.click(main.getByRole('link', { name: 'Statistiques' }))
    await waitFor(() => expect(router.state.location.pathname).toBe('/stats'))
    // La barre latérale (la barre d'onglets du bas n'a que 4 onglets, sans les statistiques).
    const nav = within(screen.getAllByRole('navigation', { name: /navigation/i })[0])
    expect(nav.getByRole('link', { name: 'Statistiques' })).toHaveAttribute('href', '/stats')
  })
})
