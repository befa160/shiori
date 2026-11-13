import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

const libraries: Library[] = [
  { id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 0, volume_count: 0 },
  { id: 2, name: 'Webtoons', root: '/w', kind: 'webtoon', series_count: 0, volume_count: 0 },
]

afterEach(() => {
  localStorage.clear()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

// jsdom n'applique pas les media queries : sidebar et barre d'onglets sont toutes deux dans la page.
const [sidebarNav, tabBar] = [0, 1].map((i) => () => screen.getAllByRole('navigation', { name: 'Navigation principale' })[i])

describe('cadre de navigation', () => {
  it('mène aux bibliothèques et à l’administration, et marque l’écran courant', async () => {
    mockApi({ '/api/libraries': () => json(libraries) })
    const { router } = renderAt('/')
    const webtoons = await within(sidebarNav()).findByRole('link', { name: 'Webtoons' })
    // Dépliée : pas d'infobulle, donc pas de libellé lu deux fois au focus.
    within(sidebarNav()).getByRole('link', { name: 'Accueil' }).focus()
    expect(within(sidebarNav()).getByRole('link', { name: 'Accueil' })).not.toHaveAttribute('aria-describedby')
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(within(sidebarNav()).getByRole('link', { name: 'Accueil' })).toHaveAttribute('aria-current', 'page')
    expect(within(tabBar()).getByRole('link', { name: 'Accueil' })).toHaveAttribute('aria-current', 'page')

    await userEvent.click(webtoons)
    expect(router.state.location.pathname).toBe('/libraries/2')
    expect(webtoons).toHaveAttribute('aria-current', 'page')
    expect(within(sidebarNav()).getByRole('link', { name: 'Accueil' })).not.toHaveAttribute('aria-current')

    await userEvent.click(within(tabBar()).getByRole('link', { name: 'Administration' }))
    expect(router.state.location.pathname).toBe('/admin')
    expect(screen.getByRole('heading', { level: 1, name: 'Administration' })).toBeInTheDocument()
    expect(within(screen.getByRole('main')).getByRole('combobox', { name: 'Langue' })).toBeInTheDocument()
    expect(within(sidebarNav()).getByRole('link', { name: 'Administration' })).toHaveAttribute('aria-current', 'page')
  })

  it('rescanne une bibliothèque depuis la sidebar', async () => {
    const fetch = mockApi({ '/api/libraries': () => json(libraries), '/api/admin/scans': () => json({ run: 3 }, 202) })
    renderAt('/')
    await userEvent.click(await within(sidebarNav()).findByRole('button', { name: 'Scanner Webtoons' }))
    expect(await screen.findByText('Scan de « Webtoons » lancé')).toBeInTheDocument()
    const post = fetch.mock.calls.find(([url, init]) => url === '/api/admin/scans' && init?.method === 'POST')
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ library_id: 2 })
  })

  it('propose d’aller directement au contenu', () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/')
    expect(screen.getByRole('link', { name: 'Aller au contenu' })).toHaveAttribute('href', '#main')
    expect(screen.getByRole('main')).toHaveAttribute('id', 'main')
  })

  it('n’affiche pas de liste de bibliothèques vide dans la sidebar', async () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/')
    expect(await screen.findByText("Aucune bibliothèque pour l'instant.")).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: 'Bibliothèques', level: 2 })).toBeNull()
  })
})

describe('sidebar réduite', () => {
  const sidebar = () => document.querySelector('[data-slot="sidebar"]')

  it('se réduit, se déplie, et s’en souvient au prochain chargement', async () => {
    mockApi({ '/api/libraries': () => json(libraries) })
    const { unmount } = renderAt('/')
    expect(sidebar()).toHaveAttribute('data-state', 'expanded')
    await userEvent.click(screen.getByRole('button', { name: 'Réduire le menu' }))
    expect(sidebar()).toHaveAttribute('data-state', 'collapsed')
    expect(screen.getByRole('button', { name: 'Déplier le menu' })).toHaveAttribute('aria-expanded', 'false')
    // Réduite, le libellé passe dans une infobulle au focus.
    const admin = within(sidebarNav()).getByRole('link', { name: 'Administration' })
    admin.focus()
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Administration')
    unmount()

    renderAt('/')
    expect(sidebar()).toHaveAttribute('data-state', 'collapsed')
    await userEvent.click(screen.getByRole('button', { name: 'Déplier le menu' }))
    expect(sidebar()).toHaveAttribute('data-state', 'expanded')
    expect(localStorage.getItem('shiori.sidebar')).toBe('expanded')
  })

  it('reste utilisable si le navigateur refuse le stockage local', async () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('refusé', 'SecurityError')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('refusé', 'SecurityError')
    })
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/')
    expect(sidebar()).toHaveAttribute('data-state', 'expanded')
    await userEvent.click(screen.getByRole('button', { name: 'Réduire le menu' }))
    expect(sidebar()).toHaveAttribute('data-state', 'collapsed')
  })
})
