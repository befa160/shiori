import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Account, Invite, Library } from '@/api/types'
import { ADMIN, READER, json, mockApi, renderAt } from '@/test/render'

const libraries: Library[] = [
  { id: 1, name: 'Mangas', root: '/m', kind: 'manga', watch: true, series_count: 0, volume_count: 0 },
  { id: 2, name: 'Webtoons', root: '/w', kind: 'webtoon', watch: true, series_count: 0, volume_count: 0 },
]
const accounts: Account[] = [
  { ...ADMIN, libraries: [], totp: false },
  { ...READER, libraries: [1], totp: true },
  { ...READER, id: 3, username: 'tous', libraries: [1, 2], must_change_password: true, totp: false },
  { ...READER, id: 4, username: 'parti', libraries: [], disabled: true, totp: false },
]
const body = (init?: RequestInit) => JSON.parse(String(init?.body))
const base = { '/api/libraries': () => json(libraries), '/api/admin/scans/current': () => new Response(null, { status: 204 }) }

afterEach(() => vi.unstubAllGlobals())

describe('administration des comptes', () => {
  it('s’ouvre depuis l’administration et résume chaque compte', async () => {
    mockApi({ ...base, '/api/admin/users': () => json(accounts) })
    const { router } = renderAt('/admin')
    await userEvent.click(await screen.findByRole('link', { name: 'Gérer les comptes' }))
    expect(router.state.location.pathname).toBe('/admin/users')
    const main = within(screen.getByRole('main'))
    expect(await main.findByText('Administrateur · toutes les bibliothèques')).toBeInTheDocument()
    expect(main.getByText('Lecteur · 1 bibliothèque · double authentification')).toBeInTheDocument()
    expect(main.getByText('Lecteur · toutes les bibliothèques · mot de passe provisoire')).toBeInTheDocument()
    expect(main.getByText('Lecteur · 0 bibliothèque · désactivé')).toBeInTheDocument()
  })

  it('crée un lecteur (toutes les bibliothèques cochées d’avance) et montre son mot de passe une fois', async () => {
    let tries = 0
    const fetch = mockApi({
      ...base,
      '/api/admin/users': (init) => {
        if (init?.method !== 'POST') return json(accounts)
        if (tries++ === 0) return json({ code: 'name_taken', message: 'x' }, 409)
        return json({ user: { ...READER, username: 'léa', libraries: [2] }, password: 'MOT-PROVISOIRE' }, 201)
      },
    })
    const writeText = vi.fn(async () => {})
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText } })
    renderAt('/admin/users')
    await userEvent.click(screen.getByRole('button', { name: 'Nouveau compte' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Nom du compte'), 'léa')
    expect(dialog.getByRole('checkbox', { name: 'Mangas' })).toBeChecked()
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Mangas' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le compte' }))
    expect(await dialog.findByText('Un compte porte déjà ce nom')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le compte' }))
    expect(await dialog.findByText('MOT-PROVISOIRE')).toBeInTheDocument()
    const post = fetch.mock.calls.filter(([url, init]) => url === '/api/admin/users' && init?.method === 'POST').at(-1)
    expect(body(post?.[1])).toEqual({ username: 'léa', role: 'reader', libraries: [2] })
    await userEvent.click(dialog.getByRole('button', { name: 'Copier le mot de passe' }))
    expect(writeText).toHaveBeenCalledWith('MOT-PROVISOIRE')
    expect(await screen.findByText('Mot de passe copié')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })

  it('un administrateur se crée sans liste de bibliothèques ; copie refusée : le dit', async () => {
    const fetch = mockApi({
      ...base,
      '/api/admin/users': (init) =>
        init?.method === 'POST' ? json({ user: { ...ADMIN, id: 9, username: 'bob', libraries: [] }, password: 'P' }, 201) : json([]),
    })
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText: async () => Promise.reject(new Error('refusé')) } })
    renderAt('/admin/users')
    await userEvent.click(screen.getByRole('button', { name: 'Nouveau compte' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.type(dialog.getByLabelText('Nom du compte'), 'bob')
    await userEvent.click(dialog.getByRole('combobox', { name: 'Rôle' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Administrateur' }))
    expect(dialog.getByText(/voit toutes les bibliothèques/)).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le compte' }))
    await userEvent.click(await dialog.findByRole('button', { name: 'Copier le mot de passe' }))
    expect(await screen.findByText('Copie impossible : sélectionnez le mot de passe')).toBeInTheDocument()
    const post = fetch.mock.calls.find(([, init]) => init?.method === 'POST')
    expect(body(post?.[1])).toEqual({ username: 'bob', role: 'admin' })
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
  })

  it('modifie un lecteur : bibliothèques, désactivation ; refus du serveur en tête', async () => {
    let tries = 0
    const fetch = mockApi({
      ...base,
      '/api/admin/users': () => json(accounts),
      '/api/admin/users/2': () => (tries++ === 0 ? json({ code: 'last_admin', message: 'x' }, 409) : json(accounts[1])),
    })
    renderAt('/admin/users')
    await userEvent.click(await screen.findByRole('button', { name: 'Modifier lecteur' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Webtoons' }))
    await userEvent.click(dialog.getByRole('checkbox', { name: 'Compte désactivé' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Enregistrer' }))
    expect(await dialog.findByRole('alert')).toHaveTextContent('Il faut garder au moins un administrateur actif')
    await userEvent.click(dialog.getByRole('button', { name: 'Enregistrer' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    const patch = fetch.mock.calls.filter(([url]) => url === '/api/admin/users/2').at(-1)
    expect(patch?.[1]?.method).toBe('PATCH')
    expect(body(patch?.[1])).toEqual({ role: 'reader', disabled: true, libraries: [1, 2] })
  })

  it('nouveau mot de passe, déconnexion des appareils, suppression, et leurs échecs', async () => {
    let fail = false
    const fetch = mockApi({
      ...base,
      '/api/admin/users': () => json(accounts),
      '/api/admin/users/2/password': () => (fail ? json({ code: 'internal', message: 'x' }, 500) : json({ password: 'NOUVEAU' })),
      '/api/admin/users/2/sessions': () => (fail ? json({ code: 'internal', message: 'x' }, 500) : new Response(null, { status: 204 })),
      '/api/admin/users/2/totp': () => (fail ? json({ code: 'internal', message: 'x' }, 500) : new Response(null, { status: 204 })),
      '/api/admin/users/2': () => (fail ? json({ code: 'internal', message: 'x' }, 500) : new Response(null, { status: 204 })),
    })
    renderAt('/admin/users')
    const open = async () => {
      await userEvent.click(await screen.findByRole('button', { name: 'Modifier lecteur' }))
      return within(await screen.findByRole('dialog'))
    }
    let dialog = await open()
    await userEvent.click(dialog.getByRole('button', { name: 'Retirer la double authentification' }))
    expect(await screen.findByText('Double authentification de lecteur retirée')).toBeInTheDocument()
    expect(dialog.queryByRole('button', { name: 'Retirer la double authentification' })).toBeNull()
    await userEvent.click(dialog.getByRole('button', { name: 'Déconnecter tous ses appareils et jetons' }))
    expect(await screen.findByText('Appareils et jetons de lecteur coupés')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Nouveau mot de passe provisoire' }))
    expect(await dialog.findByText('NOUVEAU')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
    dialog = await open()
    await userEvent.click(dialog.getByRole('button', { name: 'Supprimer le compte' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Compte lecteur supprimé')).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith('/api/admin/users/2', expect.objectContaining({ method: 'DELETE' }))

    fail = true
    dialog = await open()
    await userEvent.click(dialog.getByRole('button', { name: 'Retirer la double authentification' }))
    expect(await screen.findByText('Double authentification non retirée')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Déconnecter tous ses appareils et jetons' }))
    expect(await screen.findByText('Appareils non déconnectés')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Nouveau mot de passe provisoire' }))
    expect(await screen.findByText('Mot de passe non changé')).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Supprimer le compte' }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Compte non supprimé')).toBeInTheDocument()
  })

  it('son propre compte : ni désactivation, ni suppression ; sans bibliothèque, le dit', async () => {
    mockApi({ '/api/libraries': () => json([]), '/api/admin/scans/current': () => new Response(null, { status: 204 }), '/api/admin/users': () => json(accounts) })
    renderAt('/admin/users')
    await userEvent.click(await screen.findByRole('button', { name: 'Modifier admin' }))
    const dialog = within(await screen.findByRole('dialog'))
    expect(dialog.queryByRole('checkbox', { name: 'Compte désactivé' })).toBeNull()
    expect(dialog.queryByRole('button', { name: 'Supprimer le compte' })).toBeNull()
    await userEvent.click(dialog.getByRole('combobox', { name: 'Rôle' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Lecteur' }))
    expect(dialog.getByText("Aucune bibliothèque pour l'instant.")).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Annuler' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })

  it('invite : lien montré une fois, invitations en attente (périmée signalée), révocation confirmée', async () => {
    let invites: Invite[] = [{ id: 9, role: 'reader', libraries: [1], created_at: '2026-01-01T00:00:00Z', expires_at: '2026-01-08T00:00:00Z' }]
    const fetch = mockApi({
      ...base,
      '/api/admin/users': () => json(accounts),
      '/api/admin/invites': (init) => {
        if (init?.method !== 'POST') return json(invites)
        const created = { id: 10, role: 'admin' as const, libraries: [], created_at: '2099-01-01T00:00:00Z', expires_at: '2099-01-31T00:00:00Z' }
        invites = [created, ...invites]
        return json({ ...created, token: 'JETON' }, 201)
      },
      '/api/admin/invites/9': () => {
        invites = invites.filter((i) => i.id !== 9)
        return new Response(null, { status: 204 })
      },
    })
    const writeText = vi.fn(async () => {})
    vi.stubGlobal('navigator', { ...navigator, onLine: true, languages: ['fr'], clipboard: { writeText } })
    renderAt('/admin/users')
    const pending = within(await screen.findByRole('region', { name: 'Invitations en attente' }))
    expect(pending.getByText('Lecteur · 1 bibliothèque')).toBeInTheDocument()
    expect(pending.getByText(/Expirée le/)).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Inviter' }))
    const dialog = within(await screen.findByRole('dialog'))
    await userEvent.click(dialog.getByRole('combobox', { name: 'Rôle' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Administrateur' }))
    await userEvent.click(dialog.getByRole('combobox', { name: 'Lien valable' }))
    await userEvent.click(await screen.findByRole('option', { name: '30 jours' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Créer le lien' }))
    const link = `${window.location.origin}/invite/JETON`
    expect(await dialog.findByText(link)).toBeInTheDocument()
    const post = fetch.mock.calls.filter(([url, init]) => url === '/api/admin/invites' && init?.method === 'POST').at(-1)
    expect(body(post?.[1])).toEqual({ role: 'admin', days: 30 })
    await userEvent.click(dialog.getByRole('button', { name: 'Copier le lien' }))
    expect(writeText).toHaveBeenCalledWith(link)
    await userEvent.click(dialog.getByRole('button', { name: "C'est noté" }))
    expect(await pending.findByText(/Valable jusqu'au/)).toBeInTheDocument()

    await userEvent.click(pending.getByRole('button', { name: /Révoquer l'invitation Lecteur/ }))
    await userEvent.click(await screen.findByRole('button', { name: 'Révoquer' }))
    await waitFor(() => expect(pending.queryByText(/Expirée le/)).not.toBeInTheDocument())
    expect(fetch.mock.calls.some(([url, init]) => url === '/api/admin/invites/9' && init?.method === 'DELETE')).toBe(true)
  })
})
