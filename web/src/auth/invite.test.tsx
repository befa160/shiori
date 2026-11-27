import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { READER, json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const body = (init?: RequestInit) => JSON.parse(String(init?.body))
const URL = '/api/auth/invite/JETON'

describe('lien d’invitation', () => {
  it('crée le compte (champ fautif signalé), ouvre la session et mène à l’accueil', async () => {
    let tries = 0
    const fetch = mockApi({
      [URL]: (init) => {
        if (init?.method !== 'POST') return json({ role: 'reader' })
        return tries++ === 0 ? json({ code: 'name_taken', message: 'x' }, 409) : json({ user: READER })
      },
      '/api/libraries': () => json([]),
      '/api/continue': () => json([]),
      '/api/series': () => json([]),
    })
    const { router } = renderAt('/invite/JETON', { auth: null })
    expect(await screen.findByRole('heading', { name: 'Créer votre compte' })).toBeInTheDocument()
    expect(screen.getByText(/invité à lire/)).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText('Nom du compte'), ' léa ')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Créer le compte' }))
    const name = screen.getByLabelText('Nom du compte')
    expect(await screen.findByText('Un compte porte déjà ce nom')).toBeInTheDocument()
    expect(name).toHaveAttribute('aria-invalid', 'true')
    await userEvent.click(screen.getByRole('button', { name: 'Créer le compte' }))
    await vi.waitFor(() => expect(router.state.location.pathname).toBe('/'))
    const post = fetch.mock.calls.filter(([url, init]) => url === URL && init?.method === 'POST').at(-1)
    expect(body(post?.[1])).toEqual({ username: 'léa', password: 'une phrase' })
  })

  it('lien expiré, utilisé ou révoqué : le dit et propose la connexion', async () => {
    mockApi({ [URL]: () => json({ code: 'invite_invalid', message: 'x' }, 404) })
    renderAt('/invite/JETON', { auth: null })
    expect(await screen.findByRole('heading', { name: "Lien d'invitation non valable" })).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('a expiré, a déjà servi ou a été révoqué')
    expect(screen.getByRole('link', { name: 'Aller à la connexion' })).toHaveAttribute('href', '/')
  })

  it('lien devenu invalide pendant la saisie (administrateur, révoqué) : même écran', async () => {
    mockApi({ [URL]: (init) => (init?.method === 'POST' ? json({ code: 'invite_invalid', message: 'x' }, 404) : json({ role: 'admin' })) })
    renderAt('/invite/JETON', { auth: null })
    expect(await screen.findByText(/comme administrateur/)).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText('Nom du compte'), 'chef')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Créer le compte' }))
    expect(await screen.findByRole('heading', { name: "Lien d'invitation non valable" })).toBeInTheDocument()
  })

  it('serveur injoignable : réessayer', async () => {
    let up = false
    mockApi({ [URL]: () => (up ? json({ role: 'reader' }) : json({ code: 'internal', message: 'x' }, 500)) })
    renderAt('/invite/JETON', { auth: null })
    await userEvent.click(await screen.findByRole('button', { name: 'Réessayer' }))
    up = true
    await userEvent.click(await screen.findByRole('button', { name: 'Réessayer' }))
    expect(await screen.findByRole('heading', { name: 'Créer votre compte' })).toBeInTheDocument()
  })
})
