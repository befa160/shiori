import { MutationCache, QueryCache, QueryClient, type QueryObserverOptions } from '@tanstack/react-query'
import { ApiError } from '@/api/client'

/** Une erreur 4xx est une réponse définitive (page cassée, tome inconnu) : jamais réessayée. */
export function shouldRetry(failures: number, error: unknown): boolean {
  if (error instanceof ApiError && error.status >= 400 && error.status < 500) return false
  return failures < 2
}

/** Session expirée ou fermée ailleurs, mot de passe à changer, ou rôle retiré (`forbidden`) : la
 * garde (`AuthGate`) et l'interface doivent le savoir. */
export function needsSignIn(error: unknown): boolean {
  return error instanceof ApiError && ['unauthenticated', 'password_change_required', 'forbidden'].includes(error.code)
}

export function createQueryClient(
  queries: Pick<QueryObserverOptions, 'retry' | 'staleTime'> = { retry: shouldRetry, staleTime: 30_000 },
): QueryClient {
  // Toute requête refusée faute de session redemande l'état de connexion : la page de connexion
  // s'affiche à la place de l'écran, à la même adresse (on y revient une fois connecté).
  const onError = (error: unknown) => {
    if (needsSignIn(error)) void client.invalidateQueries({ queryKey: ['auth'] })
  }
  const client: QueryClient = new QueryClient({
    queryCache: new QueryCache({ onError: (error, query) => query.queryKey[0] !== 'auth' && onError(error) }),
    mutationCache: new MutationCache({ onError }),
    defaultOptions: { queries },
  })
  return client
}
