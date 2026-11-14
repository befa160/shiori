import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { applyStoredTheme, applyTheme, resolveTheme } from '@/lib/theme'
import { json, mockApi, renderAt } from '@/test/render'

const html = document.documentElement

beforeEach(() => {
  delete html.dataset.theme
  html.classList.add('dark')
  localStorage.clear()
  document.head.innerHTML = '<meta name="theme-color" content="#121212" />'
})
afterEach(() => vi.unstubAllGlobals())

/** `matchMedia` dont on change la réponse à `prefers-color-scheme: light` en direct. */
function device(light: boolean) {
  const listeners = new Set<() => void>()
  const media = { matches: light }
  vi.stubGlobal('matchMedia', () => ({
    get matches() {
      return media.matches
    },
    addEventListener: (_: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_: string, listener: () => void) => listeners.delete(listener),
  }))
  return (next: boolean) => {
    media.matches = next
    for (const listener of listeners) listener()
  }
}

function serve(theme: string, fail = () => false) {
  return mockApi({
    '/api/libraries': () => json([]),
    '/api/admin/scans/current': () => new Response(null, { status: 204 }),
    '/api/series': () => json([]),
    '/api/continue': () => json([]),
    '/api/settings': (init) => {
      if (init?.method !== 'PUT') return json({ language: 'fr', page_turn: 'reading', theme })
      if (fail()) return json({ code: 'internal', message: 'x' }, 500)
      theme = JSON.parse(String(init.body)).theme
      return json({ language: 'fr', page_turn: 'reading', theme })
    },
  })
}

describe('thème', () => {
  it('« système » suit l’appareil, les autres s’imposent', () => {
    expect(resolveTheme('system', true)).toBe('light')
    expect(resolveTheme('system', false)).toBe('dark')
    expect(resolveTheme('sepia', false)).toBe('sepia')
  })

  it('pose les jetons, la classe des variantes `dark:` et le garde pour le prochain chargement', () => {
    applyTheme('light')
    expect([html.dataset.theme, html.classList.contains('dark')]).toEqual(['light', false])
    applyTheme('oled')
    expect([html.dataset.theme, html.classList.contains('dark')]).toEqual(['oled', true])
    delete html.dataset.theme
    applyStoredTheme()
    expect(html.dataset.theme).toBe('oled')
  })

  it('ignore un thème gardé inconnu ou un stockage refusé', () => {
    localStorage.setItem('shiori.theme', 'rose')
    applyStoredTheme()
    expect(html.dataset.theme).toBeUndefined()
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('refusé')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('refusé')
    })
    applyStoredTheme()
    applyTheme('sepia')
    expect(html.dataset.theme).toBe('sepia')
    vi.restoreAllMocks()
  })

  it('suit l’appareil en direct quand le réglage est « Système »', async () => {
    const setLight = device(false)
    serve('system')
    renderAt('/admin')
    await waitFor(() => expect(html.dataset.theme).toBe('dark'))
    setLight(true)
    await waitFor(() => expect(html.dataset.theme).toBe('light'))
  })

  it('se choisit dans l’administration, s’applique et dit s’il échoue', async () => {
    device(false)
    let fail = false
    const fetch = serve('dark', () => fail)
    renderAt('/admin')
    const section = within(await screen.findByRole('region', { name: 'Thème' }))
    const group = await section.findByRole('radiogroup', { name: 'Thème' })
    expect(group).toHaveAccessibleDescription(/Le lecteur reste toujours sombre/)
    expect(section.getByRole('radio', { name: 'Sombre' })).toBeChecked()
    await userEvent.click(section.getByRole('radio', { name: 'Sépia' }))
    await waitFor(() => expect(html.dataset.theme).toBe('sepia'))
    const put = fetch.mock.calls.find(([url, init]) => url === '/api/settings' && init?.method === 'PUT')
    expect(JSON.parse(String(put?.[1]?.body))).toEqual({ theme: 'sepia' })

    fail = true
    await userEvent.click(section.getByRole('radio', { name: 'Noir OLED' }))
    expect(await screen.findByText('Thème non enregistré')).toBeInTheDocument()
    // Notification en thème clair : sa description reste lisible sur le fond clair.
    expect(document.querySelector('[data-sonner-toaster]')).toHaveAttribute('data-sonner-theme', 'light')
    expect(section.getByRole('radio', { name: 'Sépia' })).toBeChecked()
    expect(html.dataset.theme).toBe('sepia')
  })
})
