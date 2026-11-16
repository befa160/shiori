import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

describe('données du serveur', () => {
  it('montre les chemins de la base et du cache, sélectionnables', async () => {
    mockApi({
      '/api/libraries': () => json([]),
      '/api/admin/scans/current': () => new Response(null, { status: 204 }),
      '/api/admin/storage': () =>
        json({ database: 'C:\\Users\\moi\\AppData\\Roaming\\shiori\\data\\shiori.db', cache: '/var/cache/shiori' }),
    })
    renderAt('/admin')
    const section = within(await screen.findByRole('region', { name: 'Données' }))
    const path = section.getByText('C:\\Users\\moi\\AppData\\Roaming\\shiori\\data\\shiori.db')
    expect(path).toHaveClass('select-all')
    expect(section.getByText('Base')).toBeInTheDocument()
    expect(section.getByText('/var/cache/shiori')).toBeInTheDocument()
  })

  it('ne montre rien tant que le serveur ne répond pas', async () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/admin')
    expect(await screen.findByRole('heading', { level: 1, name: 'Administration' })).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: 'Données' })).not.toBeInTheDocument()
  })
})
