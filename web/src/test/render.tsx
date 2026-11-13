import { QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import { vi } from 'vitest'
import type { SeriesMetadata } from '@/api/types'
import { App } from '@/App'
import { routes } from '@/routes'

/** Rendu de l'application à une URL, sans réessai des requêtes (tests déterministes). Par défaut
 * tout est redemandé à chaque affichage ; `staleTime` reprend la durée de la production. */
export function renderAt(path: string, { staleTime = 0 } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime } } })
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  return { ...render(<App router={router} queryClient={queryClient} />), router }
}

/** Remplace `fetch` : chaque URL `/api/...` reçoit la réponse donnée (qui voit la requête, pour
 * distinguer GET, PUT, PATCH, DELETE), une URL inconnue un 404.
 * Les réglages répondent « français » sauf si le test les fournit. */
export function mockApi(answers: Record<string, (init?: RequestInit) => Response | Promise<Response>>) {
  const all: Record<string, (init?: RequestInit) => Response | Promise<Response>> = {
    '/api/settings': () => json({ language: 'fr' }),
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

/** Métadonnées d'une série dont les fichiers ne disent rien. */
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
