import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ADMIN, READER, json, mockApi, renderAt } from '@/test/render'

afterEach(() => vi.unstubAllGlobals())

const libraries = () => json([])
const body = (init?: RequestInit) => JSON.parse(String(init?.body))

describe('garde de l’application', () => {
  it('sans compte, montre l’installation, signale le champ fautif, puis ouvre l’écran demandé', async () => {
    let tries = 0
    const fetch = mockApi({
      '/api/auth/state': () => json({ setup_required: true, user: null }),
      '/api/auth/setup': () =>
        tries++ === 0 ? json({ code: 'invalid_setup_token', message: 'x' }, 403) : json({ user: ADMIN }),
      '/api/libraries': libraries,
      '/api/series': libraries,
      '/api/continue': libraries,
    })
    renderAt('/series', { auth: null })
    expect(await screen.findByRole('heading', { name: 'Installation' })).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText("Jeton d'installation"), ' JETON ')
    await userEvent.type(screen.getByLabelText('Nom du compte'), 'admin')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Créer le compte' }))
    const token = screen.getByLabelText("Jeton d'installation")
    expect(await screen.findByRole('alert')).toHaveTextContent("Jeton d'installation incorrect")
    expect(token).toHaveAttribute('aria-invalid', 'true')
    expect(token).toHaveAccessibleDescription(/console de Shiori.*Jeton d'installation incorrect/)
    await userEvent.click(screen.getByRole('button', { name: 'Créer le compte' }))
    expect(await screen.findByRole('heading', { name: 'Séries', level: 1 })).toBeInTheDocument()
    const setup = fetch.mock.calls.filter(([url]) => url === '/api/auth/setup').at(-1)
    expect(body(setup?.[1])).toEqual({ token: 'JETON', username: 'admin', password: 'une phrase' })
  })

  it('installation faite ailleurs entre-temps : passe à la connexion', async () => {
    let done = false
    mockApi({
      '/api/auth/state': () => json({ setup_required: !done, user: null }),
      '/api/auth/setup': () => {
        done = true
        return json({ code: 'already_set_up', message: 'x' }, 409)
      },
    })
    renderAt('/', { auth: null })
    await userEvent.click(await screen.findByRole('button', { name: 'Créer le compte' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
  })

  it('connexion : erreur traduite, mot de passe affichable, puis retour à la page demandée', async () => {
    let tries = 0
    const fetch = mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: null }),
      '/api/auth/login': () =>
        [json({ code: 'invalid_credentials', message: 'x' }, 401), json({ code: 'rate_limited', message: 'x' }, 429), json({ user: ADMIN })][tries++],
      '/api/libraries': libraries,
      '/api/series': libraries,
    })
    renderAt('/series', { auth: null })
    await userEvent.type(await screen.findByLabelText('Nom du compte'), ' admin ')
    const password = screen.getByLabelText('Mot de passe')
    expect(password).toHaveAttribute('autocomplete', 'current-password')
    await userEvent.type(password, 'mauvais')
    await userEvent.click(screen.getByRole('button', { name: 'Afficher le mot de passe' }))
    expect(password).toHaveAttribute('type', 'text')
    await userEvent.click(screen.getByRole('button', { name: 'Masquer le mot de passe' }))
    expect(password).toHaveAttribute('type', 'password')
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Nom ou mot de passe incorrect')
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    expect(await screen.findByText(/Trop d’essais/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    expect(await screen.findByRole('heading', { name: 'Séries', level: 1 })).toBeInTheDocument()
    expect(body(fetch.mock.calls.find(([url]) => url === '/api/auth/login')?.[1])).toEqual({ username: 'admin', password: 'mauvais' })
  })

  it('double authentification : le code après le mot de passe, code faux dit, puis connecté', async () => {
    let tries = 0
    const fetch = mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: null }),
      '/api/auth/login': () => json({ totp_required: true, challenge: 'DEFI' }),
      '/api/auth/totp': () => (tries++ === 0 ? json({ code: 'invalid_code', message: 'x' }, 401) : json({ user: ADMIN })),
      '/api/libraries': libraries,
      '/api/series': libraries,
    })
    renderAt('/series', { auth: null })
    await userEvent.type(await screen.findByLabelText('Nom du compte'), 'admin')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    expect(await screen.findByRole('heading', { name: 'Double authentification' })).toBeInTheDocument()
    const code = screen.getByLabelText('Code')
    expect(code).toHaveAttribute('autocomplete', 'one-time-code')
    expect(code).toHaveFocus()
    await userEvent.type(code, '123 456')
    await userEvent.click(screen.getByRole('button', { name: 'Vérifier' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Code incorrect')
    await userEvent.click(screen.getByRole('button', { name: 'Vérifier' }))
    expect(await screen.findByRole('heading', { name: 'Séries', level: 1 })).toBeInTheDocument()
    expect(body(fetch.mock.calls.filter(([url]) => url === '/api/auth/totp').at(-1)?.[1])).toEqual({ challenge: 'DEFI', code: '123 456' })
  })

  it('double authentification : délai dépassé, retour à la connexion', async () => {
    mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: null }),
      '/api/auth/login': () => json({ totp_required: true, challenge: 'DEFI' }),
      '/api/auth/totp': () => json({ code: 'challenge_expired', message: 'x' }, 401),
    })
    renderAt('/', { auth: null })
    await userEvent.type(await screen.findByLabelText('Nom du compte'), 'admin')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    await userEvent.type(await screen.findByLabelText('Code'), '123456')
    await userEvent.click(screen.getByRole('button', { name: 'Vérifier' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Délai dépassé : reconnectez-vous')
    expect(screen.queryByLabelText('Code')).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Retour à la connexion' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
    expect(screen.getByLabelText('Mot de passe')).toHaveValue('')
  })

  it('mot de passe provisoire : à changer d’abord, confirmation vérifiée, erreurs sous le champ', async () => {
    let changed = false
    let tries = 0
    const fetch = mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: { ...READER, must_change_password: !changed } }),
      '/api/me/password': () => {
        if (tries++ === 0) return json({ code: 'wrong_password', message: 'x' }, 403)
        changed = true
        return new Response(null, { status: 204 })
      },
      '/api/libraries': libraries,
      '/api/series': libraries,
      '/api/continue': libraries,
    })
    renderAt('/', { auth: null })
    expect(await screen.findByRole('heading', { name: 'Nouveau mot de passe' })).toBeInTheDocument()
    await userEvent.type(screen.getByLabelText('Mot de passe provisoire'), 'provisoire')
    await userEvent.type(screen.getByLabelText('Nouveau mot de passe'), 'ma phrase')
    await userEvent.type(screen.getByLabelText('Nouveau mot de passe, encore une fois'), 'ma phrasE')
    await userEvent.click(screen.getByRole('button', { name: 'Changer le mot de passe' }))
    expect(await screen.findByText('Les deux mots de passe ne sont pas identiques')).toBeInTheDocument()
    expect(fetch.mock.calls.some(([url]) => url === '/api/me/password')).toBe(false)
    await userEvent.clear(screen.getByLabelText('Nouveau mot de passe, encore une fois'))
    await userEvent.type(screen.getByLabelText('Nouveau mot de passe, encore une fois'), 'ma phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Changer le mot de passe' }))
    expect(await screen.findByText('Mot de passe actuel incorrect')).toBeInTheDocument()
    expect(screen.getByLabelText('Mot de passe provisoire')).toHaveAttribute('aria-invalid', 'true')
    await userEvent.click(screen.getByRole('button', { name: 'Changer le mot de passe' }))
    expect(await screen.findByRole('heading', { name: 'Accueil', level: 1 })).toBeInTheDocument()
    expect(body(fetch.mock.calls.filter(([url]) => url === '/api/me/password').at(-1)?.[1])).toEqual({ current: 'provisoire', new: 'ma phrase' })
  })

  it('session terminée pendant l’usage : la page de connexion remplace l’écran', async () => {
    let signedIn = true
    mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: signedIn ? ADMIN : null }),
      '/api/libraries': () => {
        signedIn = false
        return json({ code: 'unauthenticated', message: 'x' }, 401)
      },
      '/api/series': libraries,
      '/api/continue': libraries,
    })
    renderAt('/')
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
  })

  it('un autre compte qui se connecte ne voit rien des réponses du précédent', async () => {
    let user = ADMIN
    let signedIn = true
    mockApi({
      '/api/auth/state': () => json({ setup_required: false, user: signedIn ? user : null }),
      '/api/libraries': () => json(user === ADMIN ? [{ id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 0, volume_count: 0 }] : []),
      '/api/series': libraries,
      '/api/continue': libraries,
      // La session de l'administrateur expire sur la page du compte ; celle du lecteur est valide.
      '/api/me/sessions': () => {
        if (user === READER) return json([])
        signedIn = false
        return json({ code: 'unauthenticated', message: 'x' }, 401)
      },
      '/api/auth/login': () => {
        user = READER
        signedIn = true
        return json({ user: READER })
      },
    })
    // Réponses gardées 30 s, comme en production : sans oubli, la liste de l'administrateur resterait.
    const { router } = renderAt('/', { staleTime: 30_000 })
    const sidebar = () => screen.getAllByRole('navigation', { name: 'Navigation principale' })[0]
    await waitFor(() => expect(within(sidebar()).getByRole('link', { name: 'Mangas' })).toBeInTheDocument())
    await router.navigate('/account')
    await userEvent.type(await screen.findByLabelText('Nom du compte'), 'lecteur')
    await userEvent.type(screen.getByLabelText('Mot de passe'), 'une phrase')
    await userEvent.click(screen.getByRole('button', { name: 'Se connecter' }))
    expect(await screen.findByRole('heading', { name: 'Compte', level: 1 })).toBeInTheDocument()
    await waitFor(() => expect(within(sidebar()).queryByRole('link', { name: 'Mangas' })).toBeNull())
  })

  it('rôle retiré ailleurs : l’administration disparaît au premier refus', async () => {
    let user = ADMIN
    mockApi({
      '/api/auth/state': () => json({ setup_required: false, user }),
      '/api/libraries': libraries,
      '/api/admin/scans/current': () => new Response(null, { status: 204 }),
      '/api/admin/users': () => {
        user = READER
        return json({ code: 'forbidden', message: 'x' }, 403)
      },
    })
    renderAt('/admin/users')
    expect(await screen.findByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
  })

  it('sans réseau, l’application s’ouvre quand même (tomes téléchargés)', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))))
    renderAt('/downloads', { auth: null })
    expect(await screen.findByRole('heading', { name: 'Tomes téléchargés', level: 1 })).toBeInTheDocument()
  })

  it('autre erreur : la dit et permet de réessayer', async () => {
    let tries = 0
    mockApi({
      '/api/auth/state': () => (tries++ === 0 ? json({ code: 'internal', message: 'x' }, 500) : json({ setup_required: false, user: null })),
    })
    renderAt('/', { auth: null })
    expect(await screen.findByRole('alert')).toHaveTextContent('Erreur interne du serveur')
    await userEvent.click(screen.getByRole('button', { name: 'Réessayer' }))
    expect(await screen.findByRole('heading', { name: 'Connexion' })).toBeInTheDocument()
  })
})

describe('lecteur (compte sans administration)', () => {
  it('ne voit ni l’administration, ni le scan, ni l’édition des fiches', async () => {
    const fetch = mockApi({
      '/api/libraries': () => json([{ id: 1, name: 'Mangas', root: '/m', kind: 'manga', series_count: 0, volume_count: 0 }]),
      '/api/series': libraries,
      '/api/continue': libraries,
    })
    renderAt('/admin', { auth: { setup_required: false, user: READER } })
    expect(await screen.findByRole('heading', { name: 'Page introuvable' })).toBeInTheDocument()
    const sidebar = screen.getAllByRole('navigation', { name: 'Navigation principale' })[0]
    expect(await within(sidebar).findByRole('link', { name: 'Mangas' })).toBeInTheDocument()
    expect(within(sidebar).queryByRole('link', { name: 'Administration' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Scanner Mangas' })).toBeNull()
    await waitFor(() => expect(fetch.mock.calls.some(([url]) => String(url).startsWith('/api/admin'))).toBe(false))
  })

  it('sans bibliothèque ouverte, l’accueil dit de demander à l’administrateur', async () => {
    mockApi({ '/api/libraries': libraries, '/api/series': libraries, '/api/continue': libraries })
    renderAt('/', { auth: { setup_required: false, user: READER } })
    expect(await screen.findByText(/demandez à l'administrateur/)).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Ajouter une bibliothèque' })).toBeNull()
  })
})
