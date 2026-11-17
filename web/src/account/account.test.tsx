import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Device } from '@/api/types'
import { fakeCaches } from '@/test/caches'
import { ADMIN, READER, json, me, mockApi, renderAt } from '@/test/render'
import { resetForTests } from '@/offline/downloads'

const devices: Device[] = [
  { id: 1, label: 'Firefox sur Linux', created_at: '2026-11-01T10:00:00Z', last_seen_at: '2026-11-17T10:00:00Z', current: true },
  { id: 2, label: 'Chrome sur Android', created_at: '2026-11-02T10:00:00Z', last_seen_at: '2026-11-10T10:00:00Z', current: false },
]
const empty = () => json([])
const body = (init?: RequestInit) => JSON.parse(String(init?.body))

beforeEach(() => {
  localStorage.clear()
  resetForTests()
})
afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('page du compte', () => {
  it('montre le compte, enregistre les préférences pour lui, mène à l’administration seulement pour l’administrateur', async () => {
    let theme: string | null = null
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/settings': () => json({ language: 'fr', theme: 'dark', page_turn: 'reading' }),
      '/api/me': (init) => {
        if (init?.method === 'PUT') theme = body(init).theme ?? theme
        return json(me(ADMIN, { theme: theme as null }))
      },
    })
    renderAt('/account')
    const main = within(screen.getByRole('main'))
    expect(main.getByRole('heading', { name: 'Compte', level: 1 })).toBeInTheDocument()
    expect(main.getByText('admin')).toBeInTheDocument()
    expect(main.getByRole('link', { name: 'Administration' })).toHaveAttribute('href', '/admin')
    expect(main.getByRole('link', { name: 'Téléchargés' })).toHaveAttribute('href', '/downloads')
    const sepia = await main.findByRole('radio', { name: 'Sépia' })
    expect(main.getByRole('radio', { name: 'Sombre' })).toHaveAttribute('aria-checked', 'true')
    await userEvent.click(sepia)
    await waitFor(() => expect(sepia).toHaveAttribute('aria-checked', 'true'))
    const put = fetch.mock.calls.find(([url, init]) => url === '/api/me' && init?.method === 'PUT')
    expect(body(put?.[1])).toEqual({ theme: 'sepia' })
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('sepia'))
    expect(main.getByRole('combobox', { name: 'Tourner les pages, par défaut' })).toHaveAccessibleDescription(/Pour ce compte/)
  })

  it('un lecteur n’a pas de lien vers l’administration', async () => {
    mockApi({ '/api/libraries': empty, '/api/me/sessions': () => json(devices) })
    renderAt('/account', { auth: { setup_required: false, user: READER } })
    const main = within(screen.getByRole('main'))
    expect(main.getByText(/Lecteur/)).toBeInTheDocument()
    expect(main.queryByRole('link', { name: 'Administration' })).toBeNull()
  })

  it('liste les appareils et en déconnecte un autre que celui-ci', async () => {
    let list = devices
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(list),
      '/api/me/sessions/2': () => {
        list = devices.slice(0, 1)
        return new Response(null, { status: 204 })
      },
    })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: 'Appareils connectés' }))
    expect(await region.findByText('Cet appareil')).toBeInTheDocument()
    expect(region.getAllByRole('button', { name: 'Déconnecter' })).toHaveLength(1)
    await userEvent.click(region.getByRole('button', { name: 'Déconnecter' }))
    await waitFor(() => expect(region.queryByText('Chrome sur Android')).toBeNull())
    expect(fetch).toHaveBeenCalledWith('/api/me/sessions/2', expect.objectContaining({ method: 'DELETE' }))
  })

  it('dit quand un appareil ne se déconnecte pas', async () => {
    mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/sessions/2': () => json({ code: 'internal', message: 'x' }, 500),
    })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: 'Appareils connectés' }))
    await userEvent.click(await region.findByRole('button', { name: 'Déconnecter' }))
    expect(await screen.findByText('Appareil non déconnecté')).toBeInTheDocument()
  })

  it('change le mot de passe et le confirme', async () => {
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/password': () => new Response(null, { status: 204 }),
    })
    renderAt('/account')
    const region = within(screen.getByRole('region', { name: 'Changer le mot de passe' }))
    await userEvent.type(region.getByLabelText('Mot de passe actuel'), 'ancien')
    await userEvent.type(region.getByLabelText('Nouveau mot de passe'), 'nouveau mot')
    await userEvent.type(region.getByLabelText('Nouveau mot de passe, encore une fois'), 'nouveau mot')
    await userEvent.click(region.getByRole('button', { name: 'Changer le mot de passe' }))
    expect(await screen.findByText('Mot de passe changé, autres appareils déconnectés')).toBeInTheDocument()
    expect(region.getByLabelText('Mot de passe actuel')).toHaveValue('')
    expect(body(fetch.mock.calls.find(([url]) => url === '/api/me/password')?.[1])).toEqual({ current: 'ancien', new: 'nouveau mot' })
  })

  it('montre en tête une erreur sans champ (serveur en panne)', async () => {
    mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/password': () => json({ code: 'internal', message: 'x' }, 500),
    })
    renderAt('/account')
    const region = within(screen.getByRole('region', { name: 'Changer le mot de passe' }))
    await userEvent.click(region.getByRole('button', { name: 'Changer le mot de passe' }))
    expect(await region.findByRole('alert')).toHaveTextContent('Erreur interne du serveur')
  })

  it('se déconnecte : vide le cache des pages, efface les tomes téléchargés si demandé, revient à la connexion', async () => {
    const kept = fakeCaches()
    const cache = await caches.open('shiori-images-v1')
    await cache.put('/api/pages/p1/image', new Response('x'))
    await (await caches.open('shiori-offline-v1')).put('/api/volumes/a', new Response('{}'))
    localStorage.setItem('shiori.downloads', JSON.stringify([{ volumeId: 'a', seriesId: 1, series: 'Toriko', label: 'Tome 1', pages: 1, bytes: 1 }]))
    localStorage.setItem('shiori.user', '1')
    resetForTests()
    let signedIn = true
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/auth/state': () => json({ setup_required: false, user: signedIn ? ADMIN : null }),
      '/api/auth/logout': () => {
        signedIn = false
        return new Response(null, { status: 204 })
      },
    })
    renderAt('/account')
    await userEvent.click(screen.getByRole('button', { name: 'Se déconnecter' }))
    const dialog = within(await screen.findByRole('alertdialog'))
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Effacer aussi le tome téléchargé' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Se déconnecter' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith('/api/auth/logout', expect.objectContaining({ method: 'POST' }))
    expect(kept('shiori-images-v1')).toEqual([])
    expect(kept('shiori-offline-v1')).toEqual([])
    expect(localStorage.getItem('shiori.user')).toBeNull()
  })

  it('sans réseau, la déconnexion attend ; un échec se dit', async () => {
    mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/auth/logout': () => json({ code: 'internal', message: 'x' }, 500),
    })
    const online = vi.spyOn(navigator, 'onLine', 'get').mockReturnValue(false)
    renderAt('/account')
    expect(screen.getByRole('button', { name: 'Se déconnecter' })).toBeDisabled()
    online.mockReturnValue(true)
    dispatchEvent(new Event('online'))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Se déconnecter' })).toBeEnabled())
    await userEvent.click(screen.getByRole('button', { name: 'Se déconnecter' }))
    const dialog = within(await screen.findByRole('alertdialog'))
    expect(dialog.queryByRole('checkbox')).toBeNull()
    await userEvent.click(dialog.getByRole('button', { name: 'Se déconnecter' }))
    expect(await screen.findByText('Déconnexion impossible')).toBeInTheDocument()
  })
})
