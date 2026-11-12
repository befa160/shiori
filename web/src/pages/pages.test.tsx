import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { LoadFailed } from '@/pages/NotFound'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

describe('page introuvable', () => {
  it("propose de revenir à l'accueil", async () => {
    mockApi({ '/api/libraries': () => json([]) })
    const { router } = renderAt('/nulle/part')
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: "Retour à l'accueil" }))
    expect(router.state.location.pathname).toBe('/')
    expect(await screen.findByRole('heading', { name: 'Accueil', level: 1 })).toBeInTheDocument()
  })
})

describe('écran introuvable à charger', () => {
  it('le dit et recharge la nouvelle version', async () => {
    const reload = vi.fn()
    vi.stubGlobal('location', { ...window.location, reload })
    render(<LoadFailed />)
    expect(screen.getByRole('heading', { name: "Cet écran n'a pas pu s'afficher" })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Recharger' }))
    expect(reload).toHaveBeenCalled()
  })
})
