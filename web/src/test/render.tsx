import { QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import { vi } from 'vitest'
import { App } from '@/App'
import { routes } from '@/routes'

/** Rendu de l'application à une URL, sans réessai des requêtes (tests déterministes). Par défaut
 * tout est redemandé à chaque affichage ; `staleTime` reprend la durée de la production. */
export function renderAt(path: string, { staleTime = 0 } = {}) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime } } })
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  return { ...render(<App router={router} queryClient={queryClient} />), router }
}

/** Remplace `fetch` : chaque URL `/api/...` reçoit la réponse donnée, une URL inconnue un 404.
 * Les réglages répondent « français » sauf si le test les fournit. */
export function mockApi(answers: Record<string, () => Response | Promise<Response>>) {
  const all: Record<string, () => Response | Promise<Response>> = {
    '/api/settings': () => json({ language: 'fr' }),
    ...answers,
  }
  const fetch = vi.fn(async (input: RequestInfo | URL, _init?: RequestInit) => {
    const url = typeof input === 'string' ? input : input.toString()
    const answer = all[url]
    return answer ? answer() : json({ code: 'not_found', message: 'introuvable' }, 404)
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}
