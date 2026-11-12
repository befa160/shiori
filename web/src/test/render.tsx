import { QueryClient } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { createMemoryRouter } from 'react-router'
import { vi } from 'vitest'
import { App } from '@/App'
import { routes } from '@/routes'

/** Rendu de l'application à une URL, sans réessai des requêtes (tests déterministes). */
export function renderAt(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const router = createMemoryRouter(routes, { initialEntries: [path] })
  return { ...render(<App router={router} queryClient={queryClient} />), router }
}

/** Remplace `fetch` : chaque URL `/api/...` reçoit la réponse donnée, une URL inconnue un 404. */
export function mockApi(answers: Record<string, () => Response | Promise<Response>>) {
  const fetch = vi.fn(async (input: RequestInfo | URL, _init?: RequestInit) => {
    const url = typeof input === 'string' ? input : input.toString()
    const answer = answers[url]
    return answer ? answer() : json({ code: 'not_found', message: 'introuvable' }, 404)
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}
