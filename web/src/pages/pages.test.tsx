import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

describe('accueil', () => {
  it('affiche la version du serveur', async () => {
    mockApi({ '/api/health': () => json({ status: 'ok', version: '0.1.0' }) })
    renderAt('/')
    expect(screen.getByText('Connexion au serveur…')).toBeInTheDocument()
    expect(await screen.findByText('Serveur Shiori 0.1.0')).toBeInTheDocument()
  })

  it('dit clairement que le serveur ne répond pas', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    renderAt('/')
    expect(await screen.findByRole('alert')).toHaveTextContent(/^Serveur injoignable$/)
  })

  it("affiche le message d'une erreur du serveur", async () => {
    mockApi({ '/api/health': () => json({ code: 'internal', message: 'erreur interne' }, 500) })
    renderAt('/')
    expect(await screen.findByRole('alert')).toHaveTextContent(/^Erreur : erreur interne$/)
  })
})

describe('page introuvable', () => {
  it("propose de revenir à l'accueil", async () => {
    mockApi({ '/api/health': () => json({ status: 'ok', version: '0.1.0' }) })
    const { router } = renderAt('/nulle/part')
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: "Retour à l'accueil" }))
    expect(router.state.location.pathname).toBe('/')
    expect(await screen.findByText('Serveur Shiori 0.1.0')).toBeInTheDocument()
  })
})
