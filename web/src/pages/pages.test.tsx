import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

describe('page introuvable', () => {
  it("propose de revenir à l'accueil", async () => {
    mockApi({ '/api/libraries': () => json([]) })
    const { router } = renderAt('/nulle/part')
    expect(screen.getByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('link', { name: "Retour à l'accueil" }))
    expect(router.state.location.pathname).toBe('/')
    expect(await screen.findByRole('heading', { name: 'Bibliothèques' })).toBeInTheDocument()
  })
})
