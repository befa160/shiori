import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import { vi } from 'vitest'
import type { AuthState, Me, SeriesMetadata, User } from '@/api/types'
import { App } from '@/App'
import { createQueryClient } from '@/lib/query'
import { routes } from '@/routes'

/** Rendu de l'application à une URL, sans réessai des requêtes (tests déterministes). Par défaut
 * tout est redemandé à chaque affichage ; `staleTime` reprend la durée de la production.
 * `auth` : état de connexion déjà connu (administrateur connecté par défaut) ; `null` pour que la
 * garde le demande à l'API. */
export function renderAt(path: string, { staleTime = 0, auth = { setup_required: false, user: ADMIN } as AuthState | null } = {}) {
  const queryClient = createQueryClient({ retry: false, staleTime })
  if (auth) queryClient.setQueryData(['auth'], auth)
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  return { ...render(<App router={router} queryClient={queryClient} />), router }
}

/** Remplace `fetch` : chaque URL `/api/...` reçoit la réponse donnée (qui voit la requête, pour
 * distinguer GET, PUT, PATCH, DELETE), une URL inconnue un 404.
 * Les réglages répondent « français » et la session est celle d'un administrateur sans
 * préférence, sauf si le test les fournit. */
export function mockApi(answers: Record<string, (init?: RequestInit) => Response | Promise<Response>>) {
  const all: Record<string, (init?: RequestInit) => Response | Promise<Response>> = {
    '/api/settings': () => json({ language: 'fr' }),
    '/api/auth/state': () => json({ setup_required: false, user: ADMIN }),
    '/api/me': () => json(me(ADMIN)),
    '/api/me/tokens': () => json([]),
    '/api/me/new': () => json([]),
    ...answers,
  }
  const fetch = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === 'string' ? input : input.toString()
    const answer = all[url]
    return answer ? answer(init) : json({ code: 'not_found', message: 'introuvable' }, 404)
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

export const ADMIN: User = {
  id: 1,
  username: 'admin',
  role: 'admin',
  must_change_password: false,
  disabled: false,
  created_at: '2026-11-17T10:00:00Z',
}

export const READER: User = { ...ADMIN, id: 2, username: 'lecteur', role: 'reader' }

export function me(user: User, preferences: Partial<Me['preferences']> = {}): Me {
  return { user, preferences: { language: null, theme: null, page_turn: null, ...preferences }, totp: false, recovery_codes_left: 0 }
}

/** Métadonnées d'une série dont les fichiers ne disent rien. */
/** Tome sans métadonnée ni champ modifié. */
export const NO_VOLUME_METADATA = { metadata: { title: null, number: null, summary: null, release_date: null, authors: [], cover_page_id: null }, locked: [] }

export const NO_METADATA: SeriesMetadata = {
  title: null,
  alt_titles: [],
  summary: null,
  authors: [],
  genres: [],
  tags: [],
  year: null,
  publisher: null,
  age_rating: null,
  publication_status: null,
  links: [],
  cover_page_id: null,
  cover_image_id: null,
  main_source: null,
  right_to_left: null,
}
