import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { FileHealth, LibrariesHealth } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const health: LibrariesHealth = {
  libraries: [
    { id: 1, name: 'Mangas', ok: 40, warning: 2, error: 1, unreadable: 1, deep_checked_at: '2026-11-10T08:00:00.000Z' },
    { id: 2, name: 'Comics', ok: 3, warning: 0, error: 0, unreadable: 0, deep_checked_at: null },
  ],
  codes: [
    { code: 'bad_crc', severity: 'error', files: 1 },
    { code: 'subfolders', severity: 'warning', files: 2 },
  ],
}
const broken: FileHealth = {
  path: '/srv/manga/A/A T01.cbz',
  library_id: 1,
  volume_id: 'v1',
  series_id: 7,
  status: 'error',
  issues: [{ severity: 'error', code: 'bad_crc', entry_name: '003.png', message: 'crc' }],
}
const unreadable: FileHealth = {
  path: '/srv/manga/B.cbz',
  library_id: 1,
  volume_id: null,
  series_id: null,
  status: 'error',
  issues: [{ severity: 'error', code: 'code_futur', entry_name: null, message: 'message du serveur' }],
}
const main = () => within(screen.getByRole('main'))
const files = (query = '') => `/api/admin/health/files?limit=50${query}`

describe('santé des bibliothèques', () => {
  it('montre l’état de chaque bibliothèque et lance une vérification en profondeur', async () => {
    const fetch = mockApi({
      '/api/admin/health': () => json(health),
      [files()]: () => json([broken, unreadable]),
      '/api/admin/scans': () => json({ run: 3 }, 202),
    })
    renderAt('/admin/health')
    expect(await main().findByRole('heading', { level: 1, name: 'Santé des bibliothèques' })).toBeInTheDocument()
    const mangas = (await main().findByRole('heading', { name: 'Mangas' })).closest('li') as HTMLElement
    expect(mangas).toHaveTextContent('Sains40Avertissements2Erreurs1Illisibles1')
    expect(mangas).toHaveTextContent(/Dernière vérification complète : 10 nov. 2026/)
    // Erreurs et illisibles en rouge ; les avertissements, sans couleur, gardent leur libellé.
    expect(within(mangas).getAllByText(/^\d+$/, { selector: 'dd.text-destructive' })).toHaveLength(2)
    const comics = main().getByRole('heading', { name: 'Comics' }).closest('li') as HTMLElement
    expect(comics).toHaveTextContent('Jamais vérifiée en profondeur')

    await userEvent.click(within(comics).getByRole('button', { name: 'Vérifier Comics en profondeur' }))
    const posted = fetch.mock.calls.find(([url, init]) => url === '/api/admin/scans' && init?.method === 'POST')
    expect(JSON.parse(String(posted?.[1]?.body))).toEqual({ library_id: 2, deep: true })
    expect(await screen.findByText('Scan de « Comics » lancé')).toBeInTheDocument()
  })

  it('liste les fichiers à réparer, traduits, avec un lien vers le tome quand il existe', async () => {
    mockApi({ '/api/admin/health': () => json(health), [files()]: () => json([broken, unreadable]) })
    renderAt('/admin/health')
    const region = within(await main().findByRole('region', { name: 'À réparer' }))
    const first = (await region.findByText('A T01.cbz')).closest('li') as HTMLElement
    expect(within(first).getByText('Erreur')).toHaveClass('text-destructive')
    expect(first).toHaveTextContent('/srv/manga/A/A T01.cbz')
    expect(first).toHaveTextContent('(003.png)')
    expect(within(first).getByRole('link', { name: 'Voir le tome' })).toHaveAttribute('href', '/series/7#volume-v1')
    // Fichier illisible : pas de tome à montrer ; un code inconnu garde le message du serveur.
    const second = region.getByText('B.cbz').closest('li') as HTMLElement
    expect(within(second).queryByRole('link')).toBeNull()
    expect(second).toHaveTextContent('message du serveur')
  })

  it('filtre par bibliothèque et par problème, dans l’adresse, et charge la suite', async () => {
    const page = Array.from({ length: 50 }, (_, i) => ({ ...broken, path: `/srv/manga/A/T${String(i).padStart(2, '0')}.cbz` }))
    mockApi({
      '/api/admin/health': () => json(health),
      [files()]: () => json(page),
      [files('&after=%2Fsrv%2Fmanga%2FA%2FT49.cbz')]: () => json([unreadable]),
      [files('&library_id=2')]: () => json([]),
      [files('&library_id=2&code=bad_crc')]: () => json([]),
    })
    const { router } = renderAt('/admin/health')
    const region = within(await main().findByRole('region', { name: 'À réparer' }))
    await userEvent.click(await region.findByRole('button', { name: 'Charger la suite' }))
    expect(await region.findByText('B.cbz')).toBeInTheDocument()
    expect(region.queryByRole('button', { name: 'Charger la suite' })).toBeNull()

    await userEvent.click(region.getByRole('combobox', { name: 'Bibliothèque' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Comics' }))
    expect(router.state.location.search).toBe('?library=2')
    expect(await region.findByText('Rien à réparer : aucun tome en avertissement ou en erreur.')).toBeInTheDocument()

    await userEvent.click(region.getByRole('combobox', { name: 'Problème' }))
    const options = await screen.findAllByRole('option')
    expect(options.map((o) => o.textContent)).toEqual(['Tous les problèmes', expect.any(String), expect.any(String)])
    await userEvent.click(options[1])
    expect(router.state.location.search).toBe('?library=2&code=bad_crc')
    await userEvent.click(region.getByRole('combobox', { name: 'Problème' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Tous les problèmes' }))
    await waitFor(() => expect(router.state.location.search).toBe('?library=2'))
  })

  it('est annoncée depuis l’administration', async () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/admin')
    const card = within(await main().findByRole('region', { name: 'Santé des bibliothèques' }))
    expect(card.getByRole('link', { name: 'Voir la santé' })).toHaveAttribute('href', '/admin/health')
  })
})
