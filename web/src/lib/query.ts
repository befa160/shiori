import { QueryClient } from '@tanstack/react-query'
import { ApiError } from '@/api/client'

/** Une erreur 4xx est une réponse définitive (page cassée, tome inconnu) : jamais réessayée. */
export function shouldRetry(failures: number, error: unknown): boolean {
  if (error instanceof ApiError && error.status >= 400 && error.status < 500) return false
  return failures < 2
}

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: { queries: { retry: shouldRetry, staleTime: 30_000 } },
  })
}
