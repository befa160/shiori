import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const FILE = { name: 'shiori-20261118-030000.db', size: 2_500_000, modified_ms: Date.UTC(2026, 10, 18, 3) }
const LISTING = { dir: '/data/backups', every_hours: 24, keep: 7, files: [FILE] }

function server(answers: Record<string, (init?: RequestInit) => Response | Promise<Response>> = {}) {
  return mockApi({
    '/api/libraries': () => json([]),
    '/api/admin/scans/current': () => new Response(null, { status: 204 }),
    '/api/admin/storage': () => json({ database: '/data/shiori.db', cache: '/cache' }),
    '/api/admin/backups': () => json(LISTING),
    ...answers,
  })
}

async function section() {
  return within(await screen.findByRole('region', { name: 'Données' }))
}

describe('données du serveur', () => {
  it('montre les chemins de la base et du cache, sélectionnables', async () => {
    mockApi({
      '/api/libraries': () => json([]),
      '/api/admin/scans/current': () => new Response(null, { status: 204 }),
      '/api/admin/storage': () =>
        json({ database: 'C:\\Users\\moi\\AppData\\Roaming\\shiori\\data\\shiori.db', cache: '/var/cache/shiori' }),
    })
    renderAt('/admin')
    const s = await section()
    const path = s.getByText('C:\\Users\\moi\\AppData\\Roaming\\shiori\\data\\shiori.db')
    expect(path).toHaveClass('select-all')
    expect(s.getByText('Base')).toBeInTheDocument()
    expect(s.getByText('/var/cache/shiori')).toBeInTheDocument()
    // Sauvegardes indisponibles : seulement les chemins.
    expect(s.queryByText('Sauvegardes')).not.toBeInTheDocument()
  })

  it('ne montre rien tant que le serveur ne répond pas', async () => {
    mockApi({ '/api/libraries': () => json([]) })
    renderAt('/admin')
    expect(await screen.findByRole('heading', { level: 1, name: 'Administration' })).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: 'Données' })).not.toBeInTheDocument()
  })
})

describe('sauvegardes', () => {
  it('liste le dossier, la fréquence et chaque sauvegarde à télécharger', async () => {
    server()
    renderAt('/admin')
    const s = await section()
    expect(await s.findByText('/data/backups')).toBeInTheDocument()
    expect(s.getByText('Une sauvegarde automatique toutes les 24 heures, les 7 plus récentes gardées.')).toBeInTheDocument()
    const row = within(s.getByRole('list', { name: 'Sauvegardes de la base' })).getByRole('listitem')
    expect(within(row).getByText(FILE.name)).toBeInTheDocument()
    expect(within(row).getByText(/2026.*3 Mo/)).toBeInTheDocument()
    expect(within(row).getByRole('link', { name: `Télécharger ${FILE.name}` })).toHaveAttribute('href', `/api/admin/backups/${FILE.name}`)
  })

  it('dit quand rien n’est sauvegardé automatiquement', async () => {
    server({ '/api/admin/backups': () => json({ ...LISTING, every_hours: null, files: [] }) })
    renderAt('/admin')
    const s = await section()
    expect(await s.findByText(/Pas de sauvegarde automatique/)).toBeInTheDocument()
    expect(s.getByText('Aucune sauvegarde pour le moment.')).toBeInTheDocument()
    expect(s.queryByRole('list')).not.toBeInTheDocument()
  })

  it('sauvegarde à la demande, et dit pourquoi elle échoue', async () => {
    let fail = false
    const fetch = server({
      '/api/admin/backups': (init) =>
        init?.method !== 'POST' ? json(LISTING) : fail ? json({ code: 'internal', message: 'disque plein' }, 500) : json(FILE, 201),
    })
    renderAt('/admin')
    const s = await section()
    await userEvent.click(await s.findByRole('button', { name: 'Sauvegarder maintenant' }))
    await waitFor(() => expect(fetch.mock.calls.filter(([, i]) => i?.method === 'POST')).toHaveLength(1))
    fail = true
    await userEvent.click(s.getByRole('button', { name: 'Sauvegarder maintenant' }))
    expect(await screen.findByText('Sauvegarde impossible')).toBeInTheDocument()
  })

  it('restaure une sauvegarde de la liste après confirmation, puis renvoie à la connexion', async () => {
    const fetch = server({ [`/api/admin/backups/${FILE.name}/restore`]: () => new Response(null, { status: 204 }) })
    renderAt('/admin')
    const s = await section()
    await userEvent.click(await s.findByRole('button', { name: `Restaurer ${FILE.name}` }))
    const dialog = within(await screen.findByRole('alertdialog'))
    expect(dialog.getByText(/tout le monde devra se reconnecter/)).toBeInTheDocument()
    await userEvent.click(dialog.getByRole('button', { name: 'Restaurer' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith(`/api/admin/backups/${FILE.name}/restore`, expect.objectContaining({ method: 'POST' }))
  })

  it('restaure depuis un fichier : téléversé, puis restauré, refus expliqué', async () => {
    const fetch = server({
      '/api/admin/backups/upload': () => json({ ...FILE, name: 'shiori-importee-1.db' }),
      '/api/admin/backups/shiori-importee-1.db/restore': () =>
        json({ code: 'invalid_backup', message: 'ce fichier…' }, 422),
    })
    renderAt('/admin')
    const s = await section()
    await s.findByRole('button', { name: 'Restaurer depuis un fichier…' })
    const file = new File(['SQLite format 3'], 'maison.db')
    await userEvent.upload(s.getByLabelText('Restaurer depuis un fichier…'), file)
    const dialog = within(await screen.findByRole('alertdialog', { name: 'Restaurer maison.db ?' }))
    await userEvent.click(dialog.getByRole('button', { name: 'Restaurer' }))
    expect(await screen.findByText("Ce fichier n'est pas une sauvegarde de Shiori lisible")).toBeInTheDocument()
    const upload = fetch.mock.calls.find(([url]) => url === '/api/admin/backups/upload')
    expect(upload?.[1]?.body).toBe(file)
    // Un fichier n'est pas du JSON : le navigateur donne son type.
    expect(upload?.[1]?.headers).toEqual({ accept: 'application/json' })
  })

  it('annuler le choix d’un fichier ne restaure rien', async () => {
    const fetch = server()
    renderAt('/admin')
    const s = await section()
    await s.findByRole('button', { name: 'Restaurer depuis un fichier…' })
    await userEvent.upload(s.getByLabelText('Restaurer depuis un fichier…'), new File(['x'], 'maison.db'))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Annuler' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(fetch.mock.calls.some(([url]) => url === '/api/admin/backups/upload')).toBe(false)
  })

  it('supprime une sauvegarde après confirmation', async () => {
    let deleted = false
    const fetch = server({
      '/api/admin/backups': () => json({ ...LISTING, files: deleted ? [] : [FILE] }),
      [`/api/admin/backups/${FILE.name}`]: () => {
        deleted = true
        return new Response(null, { status: 204 })
      },
    })
    renderAt('/admin')
    const s = await section()
    await userEvent.click(await s.findByRole('button', { name: `Supprimer ${FILE.name}` }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await s.findByText('Aucune sauvegarde pour le moment.')).toBeInTheDocument()
    expect(fetch).toHaveBeenCalledWith(`/api/admin/backups/${FILE.name}`, expect.objectContaining({ method: 'DELETE' }))
  })

  it('dit pourquoi une suppression échoue', async () => {
    server({ [`/api/admin/backups/${FILE.name}`]: () => json({ code: 'not_found', message: 'sauvegarde introuvable' }, 404) })
    renderAt('/admin')
    const s = await section()
    await userEvent.click(await s.findByRole('button', { name: `Supprimer ${FILE.name}` }))
    await userEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Supprimer' }))
    expect(await screen.findByText('Sauvegarde non supprimée')).toBeInTheDocument()
  })
})
