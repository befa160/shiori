import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { MetadataOverview, Settings } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const idle = { running: false, done: 0, total: 0, stopped: null }

function serve(overview: Partial<MetadataOverview>, settings: Partial<Settings> = {}) {
  let current: Settings = { language: 'fr', page_turn: 'reading', metadata_online: false, metadata_sources: null, metadata_adult: false, ...settings }
  let state: MetadataOverview = { online: current.metadata_online, sources: ['mangadex', 'anilist'], identify: idle, issues: [], ...overview }
  const fetch = mockApi({
    '/api/libraries': () => json([]),
    '/api/admin/scans/current': () => new Response(null, { status: 204 }),
    '/api/settings': (init) => {
      if (init?.method === 'PUT') {
        current = { ...current, ...JSON.parse(String(init.body)) }
        state = { ...state, online: current.metadata_online, sources: current.metadata_sources ?? ['mangadex', 'anilist'] }
      }
      return json(current)
    },
    '/api/admin/metadata': () => json(state),
    '/api/series': () => json([]),
    '/api/admin/metadata/identify': () => {
      state = { ...state, identify: { running: true, done: 3, total: 12, stopped: null } }
      return new Response(null, { status: 202 })
    },
  })
  const sent = (url: string) =>
    fetch.mock.calls.filter(([u, init]) => u === url && init?.method && init.method !== 'GET').map(([, init]) => (init?.body ? JSON.parse(String(init.body)) : null))
  const finish = () => {
    state = { ...state, identify: { running: false, done: 12, total: 12, stopped: null } }
  }
  return { sent, fetch, finish }
}

const section = async () => within(await screen.findByRole('region', { name: 'Métadonnées web' }))

describe('administration : métadonnées web', () => {
  it('explique ce qui part, s’active, puis lance une identification et en montre la progression', async () => {
    const { sent } = serve({})
    renderAt('/admin')
    const s = await section()
    const online = await s.findByRole('checkbox', { name: 'Autoriser les métadonnées web' })
    expect(online).not.toBeChecked()
    expect(online).toHaveAccessibleDescription(/Seul le titre de la série part/)
    const identify = s.getByRole('button', { name: 'Identifier les séries' })
    expect(identify).toBeDisabled()

    await userEvent.click(online)
    await waitFor(() => expect(online).toBeChecked())
    expect(sent('/api/settings')).toEqual([{ metadata_online: true }])
    // Fiches pour adultes : exclues par défaut, la case les permet.
    const adult = s.getByRole('checkbox', { name: 'Inclure les fiches pour adultes' })
    expect(adult).not.toBeChecked()
    expect(adult).toHaveAccessibleDescription(/Exclues par défaut/)
    await userEvent.click(adult)
    await waitFor(() => expect(adult).toBeChecked())
    expect(sent('/api/settings').at(-1)).toEqual({ metadata_adult: true })
    await waitFor(() => expect(identify).toBeEnabled())
    await userEvent.click(identify)
    expect(await screen.findByText('Identification lancée')).toBeInTheDocument()
    expect(await s.findByText('3 sur 12 recherches')).toBeInTheDocument()
    expect(s.getByRole('progressbar', { name: 'Identifier les séries' })).toBeInTheDocument()
    // Crédits des sources (MangaDex l'exige).
    expect(s.getByRole('link', { name: 'MangaDex' })).toHaveAttribute('href', 'https://mangadex.org')
    expect(s.getByRole('link', { name: 'AniList' })).toHaveAttribute('rel', 'noreferrer noopener')
  })

  it('à la fin d’une identification, la liste des séries, encore fraîche, est redemandée', async () => {
    const { fetch, finish } = serve({ online: true, identify: { running: true, done: 1, total: 4, stopped: null } }, { metadata_online: true })
    // La liste des séries d'abord, gardée 30 s comme en production.
    const { router } = renderAt('/series', { staleTime: 30_000 })
    await screen.findByRole('heading', { level: 1, name: 'Séries' })
    await act(() => router.navigate('/admin'))
    const s = await section()
    expect(await s.findByText('1 sur 4 recherches')).toBeInTheDocument()
    finish()
    expect(await s.findByText('Dernière identification : 12 recherches.', {}, { timeout: 3000 })).toBeInTheDocument()
    const asked = () => fetch.mock.calls.filter(([url]) => url === '/api/series').length
    const before = asked()
    await act(() => router.navigate('/series'))
    await waitFor(() => expect(asked()).toBeGreaterThan(before))
  })

  it('change l’ordre des sources aux boutons, et revient à l’ordre par défaut', async () => {
    const { sent } = serve({}, { metadata_online: true })
    renderAt('/admin')
    const s = await section()
    const order = () => within(s.getByRole('list')).getAllByRole('listitem').map((li) => li.textContent)
    await waitFor(() => expect(order()).toEqual(['1.MangaDex', '2.AniList']))
    expect(s.getByText(/Par défaut, d'après la langue de l'interface/)).toBeInTheDocument()
    expect(s.getByRole('button', { name: 'Monter MangaDex' })).toBeDisabled()
    expect(s.getByRole('button', { name: 'Descendre AniList' })).toBeDisabled()
    await userEvent.click(s.getByRole('button', { name: 'Monter AniList' }))
    await waitFor(() => expect(order()).toEqual(['1.AniList', '2.MangaDex']))
    await userEvent.click(await s.findByRole('button', { name: "Revenir à l'ordre par défaut" }))
    await waitFor(() => expect(order()).toEqual(['1.MangaDex', '2.AniList']))
    expect(sent('/api/settings')).toEqual([{ metadata_sources: ['anilist', 'mangadex'] }, { metadata_sources: null }])
  })

  it('dit pourquoi une identification s’est arrêtée, et liste ce qui reste à vérifier', async () => {
    serve({
      online: true,
      identify: { running: false, done: 4, total: 9, stopped: 'rate_limited' },
      issues: [
        { series_id: 3, series_name: 'Monster', source: 'anilist', candidates: 10, last_error: null },
        { series_id: 5, series_name: 'Akira', source: 'mangadex', candidates: 0, last_error: 'offline' },
        { series_id: 6, series_name: 'Inconnue', source: 'anilist', candidates: 0, last_error: null },
        { series_id: 7, series_name: 'Bizarre', source: 'anilist', candidates: 0, last_error: 'nouveau_code' },
      ],
    }, { metadata_online: true })
    renderAt('/admin')
    const s = await section()
    expect(await s.findByText(/une source demande d’attendre\. Relancez-la dans une minute/)).toBeInTheDocument()
    const rows = within(s.getByRole('heading', { name: 'À vérifier' }).parentElement as HTMLElement).getAllByRole('link')
    expect(rows.map((a) => [a.textContent, a.getAttribute('href')])).toEqual([
      ['MonsterAniList · 10 fiches possibles', '/series/3'],
      ['AkiraMangaDex · source injoignable', '/series/5'],
      ['InconnueAniList · Rien trouvé', '/series/6'],
      ['BizarreAniList · nouveau_code', '/series/7'],
    ])
  })

  it('autres raisons d’arrêt, fin normale, et rien à vérifier', async () => {
    for (const [stopped, text] of [
      ['offline', /une source ne répond pas/],
      ['internal', /erreur du serveur/],
      [null, 'Dernière identification : 9 recherches.'],
    ] as const) {
      serve({ online: true, identify: { running: false, done: 9, total: 9, stopped } }, { metadata_online: true })
      const { unmount } = renderAt('/admin')
      const s = await section()
      expect(await s.findByText(text)).toBeInTheDocument()
      expect(s.getByText('Rien à vérifier.')).toBeInTheDocument()
      unmount()
    }
  })
})
