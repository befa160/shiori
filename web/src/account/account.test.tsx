import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ApiToken, Device } from '@/api/types'
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
    await userEvent.click(within(screen.getByRole('main')).getByRole('button', { name: 'Se déconnecter' }))
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
    const page = within(screen.getByRole('main'))
    expect(page.getByRole('button', { name: 'Se déconnecter' })).toBeDisabled()
    online.mockReturnValue(true)
    dispatchEvent(new Event('online'))
    await waitFor(() => expect(page.getByRole('button', { name: 'Se déconnecter' })).toBeEnabled())
    await userEvent.click(page.getByRole('button', { name: 'Se déconnecter' }))
    const dialog = within(await screen.findByRole('alertdialog'))
    expect(dialog.queryByRole('checkbox')).toBeNull()
    await userEvent.click(dialog.getByRole('button', { name: 'Se déconnecter' }))
    expect(await screen.findByText('Déconnexion impossible')).toBeInTheDocument()
  })

  it('se déconnecte aussi depuis la barre latérale', async () => {
    let signedIn = true
    mockApi({
      '/api/libraries': empty,
      '/api/auth/state': () => json({ setup_required: false, user: signedIn ? ADMIN : null }),
      '/api/auth/logout': () => {
        signedIn = false
        return new Response(null, { status: 204 })
      },
    })
    renderAt('/')
    // Hors de la page Compte, seule la barre latérale le propose.
    await userEvent.click(await screen.findByRole('button', { name: 'Se déconnecter' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Se déconnecter' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
  })

  it('active la double authentification : QR en image, premier code vérifié, codes de secours montrés une fois', async () => {
    let enabled = false
    let tries = 0
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me': () => json({ ...me(ADMIN), totp: enabled, recovery_codes_left: enabled ? 10 : 0 }),
      '/api/me/totp': () => json({ secret: 'ABCDEFGHIJKLMNOP', uri: 'otpauth://totp/Shiori:admin?secret=ABCDEFGHIJKLMNOP', qr_svg: '<svg xmlns="http://www.w3.org/2000/svg"/>' }),
      '/api/me/totp/confirm': () => {
        if (tries++ === 0) return json({ code: 'invalid_code', message: 'x' }, 400)
        enabled = true
        return json({ recovery_codes: ['aaaa-bbbb-cccc', 'dddd-eeee-ffff'] })
      },
    })
    const writeText = vi.fn(async () => {})
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText } })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: 'Double authentification' }))
    expect(region.getByText(/Conseillé si Shiori est ouvert sur Internet/)).toBeInTheDocument()
    await userEvent.click(region.getByRole('button', { name: 'Activer' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Mot de passe actuel'), 'mot de passe')
    await userEvent.click(dialog.getByRole('button', { name: 'Continuer' }))
    expect(await dialog.findByRole('img', { name: /QR code/ })).toBeInTheDocument()
    expect(dialog.getByRole('img', { name: /QR code/ }).getAttribute('src')).toMatch(/^data:image\/svg\+xml;charset=utf-8,%3Csvg/)
    expect(dialog.getByText('ABCD EFGH IJKL MNOP')).toBeInTheDocument()
    expect(dialog.getByRole('link', { name: /Ouvrir dans l'application/ })).toHaveAttribute('href', 'otpauth://totp/Shiori:admin?secret=ABCDEFGHIJKLMNOP')
    await userEvent.type(dialog.getByLabelText('Code'), '000000')
    await userEvent.click(dialog.getByRole('button', { name: 'Activer' }))
    expect(await dialog.findByText('Code incorrect')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Activer' }))
    const codes = within(await screen.findByRole('list', { name: 'Codes de secours' }))
    // Déjà active derrière la fenêtre : fermer par la croix laisserait la page juste.
    expect(await region.findByText('Active. Il vous reste 10 codes de secours.')).toBeInTheDocument()
    expect(codes.getAllByRole('listitem').map((li) => li.textContent)).toEqual(['aaaa-bbbb-cccc', 'dddd-eeee-ffff'])
    await userEvent.click(screen.getByRole('button', { name: 'Copier les codes' }))
    expect(writeText).toHaveBeenCalledWith('aaaa-bbbb-cccc\ndddd-eeee-ffff')
    await userEvent.click(screen.getByRole('button', { name: "C'est noté" }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(body(fetch.mock.calls.find(([url]) => url === '/api/me/totp')?.[1])).toEqual({ password: 'mot de passe' })
    expect(body(fetch.mock.calls.filter(([url]) => url === '/api/me/totp/confirm').at(-1)?.[1])).toEqual({ code: '000000' })
  })

  it('mot de passe faux à l’activation : le dit sous le champ ; copie des codes refusée : le dit', async () => {
    let tries = 0
    mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/totp': () =>
        tries++ === 0 ? json({ code: 'wrong_password', message: 'x' }, 403) : json({ secret: 'ABCD', uri: 'otpauth://x', qr_svg: '<svg/>' }),
      '/api/me/totp/confirm': () => json({ recovery_codes: ['aaaa-bbbb-cccc'] }),
    })
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText: async () => Promise.reject(new Error('non')) } })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: 'Double authentification' }))
    await userEvent.click(region.getByRole('button', { name: 'Activer' }))
    let dialog = within(await screen.findByRole('dialog'))
    await userEvent.click(dialog.getByRole('button', { name: 'Continuer' }))
    expect(await dialog.findByText('Mot de passe actuel incorrect')).toBeInTheDocument()
    expect(dialog.getByLabelText('Mot de passe actuel')).toHaveAttribute('aria-invalid', 'true')
    await userEvent.click(dialog.getByRole('button', { name: 'Annuler' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    await userEvent.click(region.getByRole('button', { name: 'Activer' }))
    dialog = within(await screen.findByRole('dialog'))
    await userEvent.click(dialog.getByRole('button', { name: 'Continuer' }))
    await userEvent.click(await dialog.findByRole('button', { name: 'Activer' }))
    await userEvent.click(await screen.findByRole('button', { name: 'Copier les codes' }))
    expect(await screen.findByText('Copie impossible : sélectionnez le mot de passe')).toBeInTheDocument()
  })

  it('désactive la double authentification avec le mot de passe et un code ; erreurs sous le champ', async () => {
    let enabled = true
    let tries = 0
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me': () => json({ ...me(ADMIN), totp: enabled, recovery_codes_left: 1 }),
      '/api/me/totp': () => {
        tries++
        if (tries === 1) return json({ code: 'wrong_password', message: 'x' }, 403)
        if (tries === 2) return json({ code: 'invalid_code', message: 'x' }, 403)
        if (tries === 3) return json({ code: 'internal', message: 'x' }, 500)
        enabled = false
        return new Response(null, { status: 204 })
      },
    })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: 'Double authentification' }))
    expect(await region.findByText('Active. Il vous reste 1 code de secours.')).toBeInTheDocument()
    await userEvent.click(region.getByRole('button', { name: 'Désactiver' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Mot de passe actuel'), 'mot de passe')
    await userEvent.type(dialog.getByLabelText('Code'), 'aaaa-bbbb-cccc')
    const submit = dialog.getByRole('button', { name: 'Désactiver' })
    await userEvent.click(submit)
    expect(await dialog.findByText('Mot de passe actuel incorrect')).toBeInTheDocument()
    expect(dialog.getByLabelText('Mot de passe actuel')).toHaveAttribute('aria-invalid', 'true')
    await userEvent.click(submit)
    expect(await dialog.findByText('Code incorrect')).toBeInTheDocument()
    expect(dialog.getByLabelText('Code')).toHaveAttribute('aria-invalid', 'true')
    await userEvent.click(submit)
    expect(await dialog.findByRole('alert')).toHaveTextContent('Erreur interne du serveur')
    await userEvent.click(submit)
    expect(await screen.findByText('Double authentification désactivée')).toBeInTheDocument()
    expect(await region.findByRole('button', { name: 'Activer' })).toBeInTheDocument()
    const del = fetch.mock.calls.filter(([url, init]) => url === '/api/me/totp' && init?.method === 'DELETE').at(-1)
    expect(body(del?.[1])).toEqual({ password: 'mot de passe', code: 'aaaa-bbbb-cccc' })
  })

  it('jetons d’API : création (montré une fois), liste, suppression après confirmation', async () => {
    const created = { id: 7, name: 'Liseuse', prefix: 'shiori_t_ab12cd34', created_at: '2026-11-18T10:00:00Z', last_used_at: null, expires_at: '2027-11-18T10:00:00Z' }
    const old = { ...created, id: 3, name: 'Vieux', prefix: 'shiori_t_ffff0000', last_used_at: '2026-11-01T10:00:00Z', expires_at: '2020-01-01T00:00:00Z' }
    let list: ApiToken[] = [old]
    let tries = 0
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/tokens': (init) => {
        if (init?.method !== 'POST') return json(list)
        if (tries++ === 0) return json({ code: 'invalid_token_name', message: 'x' }, 400)
        list = [created, old]
        return json({ ...created, token: 'shiori_t_ab12cd34secret' }, 201)
      },
      '/api/me/tokens/3': () => {
        list = [created]
        return new Response(null, { status: 204 })
      },
    })
    const writeText = vi.fn(async () => {})
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText } })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: "Jetons d'API" }))
    expect(await region.findByText('Vieux')).toBeInTheDocument()
    expect(region.getByText('shiori_t_ffff0000…')).toBeInTheDocument()
    expect(region.getByText(/Expiré le/)).toHaveClass('text-destructive')
    await userEvent.click(region.getByRole('button', { name: 'Nouveau jeton' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Nom'), ' Liseuse ')
    await userEvent.click(dialog.getByRole('combobox', { name: 'Valable' }))
    await userEvent.click(await screen.findByRole('option', { name: '1 an' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le jeton' }))
    expect(await dialog.findByText('Nom du jeton : 1 à 64 caractères')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le jeton' }))
    expect(await dialog.findByText('shiori_t_ab12cd34secret')).toBeInTheDocument()
    const post = fetch.mock.calls.filter(([url, init]) => url === '/api/me/tokens' && init?.method === 'POST').at(-1)
    expect(body(post?.[1])).toEqual({ name: 'Liseuse', expires_in_days: 365 })
    await userEvent.click(dialog.getByRole('button', { name: 'Copier le jeton' }))
    expect(writeText).toHaveBeenCalledWith('shiori_t_ab12cd34secret')
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
    expect(await region.findByText(/^Jamais utilisé · Expire le 18 nov\. 2027/)).toBeInTheDocument()
    await userEvent.click(region.getByRole('button', { name: 'Supprimer le jeton Vieux' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    await waitFor(() => expect(region.queryByText('Vieux')).toBeNull())
  })

  it('jeton sans expiration ; échecs de copie et de suppression dits', async () => {
    const token = { id: 4, name: 'OPDS', prefix: 'shiori_t_00000000', created_at: '2026-11-18T10:00:00Z', last_used_at: null, expires_at: null }
    const fetch = mockApi({
      '/api/libraries': empty,
      '/api/me/sessions': () => json(devices),
      '/api/me/tokens': (init) => (init?.method === 'POST' ? json({ ...token, token: 'shiori_t_x' }, 201) : json([token])),
      '/api/me/tokens/4': () => json({ code: 'internal', message: 'x' }, 500),
    })
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText: async () => Promise.reject(new Error('non')) } })
    renderAt('/account')
    const region = within(await screen.findByRole('region', { name: "Jetons d'API" }))
    expect(await region.findByText('Jamais utilisé · Sans expiration')).toBeInTheDocument()
    await userEvent.click(region.getByRole('button', { name: 'Nouveau jeton' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Nom'), 'OPDS')
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le jeton' }))
    await userEvent.click(await dialog.findByRole('button', { name: 'Copier le jeton' }))
    expect(await screen.findByText('Copie impossible : sélectionnez le jeton')).toBeInTheDocument()
    expect(body(fetch.mock.calls.find(([, init]) => init?.method === 'POST')?.[1])).toEqual({ name: 'OPDS' })
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
    await userEvent.click(region.getByRole('button', { name: 'Supprimer le jeton OPDS' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Jeton non supprimé')).toBeInTheDocument()
    // Annuler le formulaire ferme sans rien créer.
    await userEvent.click(region.getByRole('button', { name: 'Nouveau jeton' }))
    await userEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Annuler' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })
})
