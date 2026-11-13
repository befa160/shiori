import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const mangas: Library = { id: 1, name: 'Mangas', root: '/srv/manga', kind: 'manga', series_count: 2, volume_count: 5 }
const main = () => within(screen.getByRole('main'))
const region = () => within(main().getByRole('region', { name: 'Bibliothèques' }))
const sent = (fetch: ReturnType<typeof mockApi>, method: string) =>
  fetch.mock.calls.filter(([, init]) => init?.method === method).map(([url, init]) => [url, init?.body ? JSON.parse(String(init.body)) : null])

describe('administration des bibliothèques', () => {
  it('liste les bibliothèques avec leur type, leurs compteurs et leur dossier', async () => {
    mockApi({ '/api/libraries': () => json([mangas]) })
    renderAt('/admin')
    const row = (await region().findByRole('heading', { name: 'Mangas' })).closest('li') as HTMLElement
    expect(row).toHaveTextContent('Manga · 2 séries · 5 tomes')
    expect(row).toHaveTextContent('/srv/manga')
  })

  it('crée une bibliothèque et lance son scan', async () => {
    const created = { id: 2, name: 'Webtoons', root: '/srv/webtoon', kind: 'webtoon' }
    const fetch = mockApi({
      '/api/libraries': (init) => (init?.method === 'POST' ? json(created, 201) : json([mangas])),
      '/api/admin/scans': () => json({ run: 7 }, 202),
    })
    renderAt('/admin')
    await userEvent.click(await region().findByRole('button', { name: 'Ajouter une bibliothèque' }))
    const dialog = within(await screen.findByRole('dialog', { name: 'Nouvelle bibliothèque' }))
    await userEvent.type(dialog.getByRole('textbox', { name: 'Nom' }), ' Webtoons ')
    await userEvent.type(dialog.getByRole('textbox', { name: 'Dossier' }), '/srv/webtoon')
    await userEvent.click(dialog.getByRole('combobox', { name: 'Type' }))
    await userEvent.click(await screen.findByRole('option', { name: 'Webtoon' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Créer et scanner' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(sent(fetch, 'POST')).toEqual([
      ['/api/libraries', { name: 'Webtoons', root: '/srv/webtoon', kind: 'webtoon' }],
      ['/api/admin/scans', { library_id: 2 }],
    ])
    expect(await screen.findByText('Bibliothèque « Webtoons » créée')).toBeInTheDocument()
    expect(await screen.findByText('Scan de « Webtoons » lancé')).toBeInTheDocument()
  })

  it('montre chaque erreur du serveur sous le champ concerné, ou en tête', async () => {
    const answers = [
      json({ code: 'invalid_root', message: '/x n’est pas un dossier' }, 400),
      json({ code: 'name_taken', message: 'déjà pris' }, 409),
      json({ code: 'internal', message: 'erreur interne' }, 500),
    ]
    mockApi({ '/api/libraries': (init) => (init?.method === 'POST' ? (answers.shift() as Response) : json([mangas])) })
    renderAt('/admin')
    await userEvent.click(await region().findByRole('button', { name: 'Ajouter une bibliothèque' }))
    const dialog = within(await screen.findByRole('dialog'))
    const root = dialog.getByRole('textbox', { name: 'Dossier' })
    const name = dialog.getByRole('textbox', { name: 'Nom' })
    const submit = dialog.getByRole('button', { name: 'Créer et scanner' })

    await userEvent.click(submit)
    expect(await dialog.findByRole('alert')).toHaveTextContent("Ce chemin n'est pas un dossier accessible au serveur")
    expect(root).toHaveAttribute('aria-invalid', 'true')
    expect(root).toHaveAccessibleDescription(/Chemin complet du dossier.*Ce chemin n'est pas un dossier/)

    await userEvent.click(submit)
    await waitFor(() => expect(name).toHaveAccessibleDescription('Une bibliothèque porte déjà ce nom'))
    expect(root).toHaveAttribute('aria-invalid', 'false')

    await userEvent.click(submit)
    await waitFor(() => expect(dialog.getByRole('alert')).toHaveTextContent('Erreur interne du serveur'))
    expect(name).toHaveAttribute('aria-invalid', 'false')
    // Annuler ferme sans rien créer.
    await userEvent.click(dialog.getByRole('button', { name: 'Annuler' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('lance un scan, et dit pourquoi il ne démarre pas', async () => {
    let busy = false
    mockApi({
      '/api/libraries': () => json([mangas]),
      '/api/admin/scans': () => (busy ? json({ code: 'scan_already_running', message: 'occupé' }, 409) : json({ run: 1 }, 202)),
    })
    renderAt('/admin')
    const scan = await region().findByRole('button', { name: 'Scanner Mangas' })
    await userEvent.click(scan)
    expect(await screen.findByText('Scan de « Mangas » lancé')).toBeInTheDocument()
    busy = true
    await userEvent.click(scan)
    expect(await screen.findByText('Scan non lancé')).toBeInTheDocument()
    expect(screen.getByText('Un scan est déjà en cours : attendez la fin, ou annulez-le')).toBeInTheDocument()
  })

  it('supprime une bibliothèque après confirmation seulement', async () => {
    let fail = true
    const fetch = mockApi({
      '/api/libraries': () => json([mangas]),
      '/api/libraries/1': () => (fail ? json({ code: 'internal', message: 'x' }, 500) : new Response(null, { status: 204 })),
    })
    renderAt('/admin')
    const remove = await region().findByRole('button', { name: 'Supprimer Mangas' })
    await userEvent.click(remove)
    const confirm = within(await screen.findByRole('alertdialog', { name: 'Supprimer « Mangas » ?' }))
    expect(confirm.getByText(/Vos fichiers ne sont pas touchés/)).toBeInTheDocument()
    await userEvent.click(confirm.getByRole('button', { name: 'Annuler' }))
    expect(sent(fetch, 'DELETE')).toEqual([])

    await userEvent.click(remove)
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Bibliothèque non supprimée')).toBeInTheDocument()
    fail = false
    await userEvent.click(remove)
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Bibliothèque « Mangas » supprimée')).toBeInTheDocument()
    expect(sent(fetch, 'DELETE')).toEqual([
      ['/api/libraries/1', null],
      ['/api/libraries/1', null],
    ])
  })

  it('sans bibliothèque, l’accueil mène à l’administration pour en ajouter une', async () => {
    mockApi({ '/api/libraries': () => json([]), '/api/series': () => json([]), '/api/continue': () => json([]) })
    const { router } = renderAt('/')
    await userEvent.click(await main().findByRole('link', { name: 'Ajouter une bibliothèque' }))
    expect(router.state.location.pathname).toBe('/admin')
    expect(await region().findByText(/Aucune bibliothèque. Ajoutez le dossier/)).toBeInTheDocument()
  })
})
