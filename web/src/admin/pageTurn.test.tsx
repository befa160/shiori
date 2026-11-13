import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

describe('réglage « Tourner les pages » par défaut', () => {
  it('s’enregistre sur le serveur sans toucher à la langue, et dit s’il échoue', async () => {
    let fail = false
    const fetch = mockApi({
      '/api/libraries': () => json([]),
      '/api/admin/scans/current': () => new Response(null, { status: 204 }),
      '/api/settings': (init) => {
        if (init?.method !== 'PUT') return json({ language: 'fr', page_turn: 'reading' })
        if (fail) return json({ code: 'internal', message: 'x' }, 500)
        return json({ language: 'fr', ...JSON.parse(String(init.body)) })
      },
    })
    renderAt('/admin')
    const section = within(await screen.findByRole('region', { name: 'Lecture' }))
    const select = await section.findByRole('combobox', { name: 'Tourner les pages, par défaut' })
    expect(select).toHaveTextContent('Comme le sens de lecture')
    expect(select).toHaveAccessibleDescription(/Le sens de lecture garde l'ordre des doubles pages/)
    await userEvent.click(select)
    await userEvent.click(await screen.findByRole('option', { name: 'Toucher à gauche avance' }))
    await waitFor(() => expect(select).toHaveTextContent('Toucher à gauche avance'))
    const put = fetch.mock.calls.find(([url, init]) => url === '/api/settings' && init?.method === 'PUT')
    expect(JSON.parse(String(put?.[1]?.body))).toEqual({ page_turn: 'rtl' })

    fail = true
    await userEvent.click(select)
    await userEvent.click(await screen.findByRole('option', { name: 'Toucher à droite avance' }))
    expect(await screen.findByText('Réglage non enregistré')).toBeInTheDocument()
    expect(select).toHaveTextContent('Toucher à gauche avance')
  })
})
