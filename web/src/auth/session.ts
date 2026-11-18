import { type QueryClient, useQuery } from '@tanstack/react-query'
import type { TFunction } from 'i18next'
import { ApiError, api } from '@/api/client'
import type { AuthState, User } from '@/api/types'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'
import { remove } from '@/offline/downloads'
import { setProgressUser } from '@/offline/progress'

/** Même nom que dans `public/sw.js` : pages et couvertures vues, propres au compte. */
const IMAGES_CACHE = 'shiori-images-v1'

// Hors ligne, l'échec réseau ouvre l'application (tomes téléchargés) : `always`, sinon React Query
// mettrait la requête en pause et la garde attendrait sans fin ; `retryOnMount: false`, sinon
// chaque écran monté la redemanderait, ce qui le démonterait (boucle). Le retour du réseau la
// redemande.
export const useAuth = () =>
  useQuery({
    queryKey: ['auth'],
    queryFn: ({ signal }) => api.authState({ signal }),
    staleTime: Infinity,
    networkMode: 'always',
    retryOnMount: false,
  })

/** Compte connecté ; `null` hors ligne (l'état de connexion n'a pas pu être demandé). */
export function useUser(): User | null {
  return useAuth().data?.user ?? null
}

export const useIsAdmin = () => useUser()?.role === 'admin'

/** Connecté (connexion ou installation) : l'écran demandé s'affiche à la place du formulaire,
 * sans rien des réponses d'un compte précédent (session expirée sur un appareil partagé). */
export function signedIn(queryClient: QueryClient, user: User) {
  forgetAccount(queryClient)
  queryClient.setQueryData<AuthState>(['auth'], { setup_required: false, user })
}

/** Oublie les réponses gardées, sauf l'état de connexion (`clear()` couperait la garde de sa requête). */
function forgetAccount(queryClient: QueryClient) {
  queryClient.removeQueries({ predicate: (query) => query.queryKey[0] !== 'auth' })
}

/** Déconnexion : ferme la session, oublie ce que l'appareil garde du compte (réponses, pages vues
 * et, si demandé, les tomes téléchargés). */
export async function signOut(queryClient: QueryClient, downloads: string[] = []) {
  await api.logout()
  if ('caches' in window) await caches.delete(IMAGES_CACHE)
  for (const volumeId of downloads) await remove(volumeId)
  signedOut(queryClient)
}

/** Session close (déconnexion, ou base restaurée par l'administrateur) : retour à la connexion. */
export function signedOut(queryClient: QueryClient) {
  setProgressUser(null)
  // La garde passe d'abord à la connexion (l'écran se démonte), puis les réponses du compte partent.
  queryClient.setQueryData<AuthState>(['auth'], { setup_required: false, user: null })
  forgetAccount(queryClient)
}

/** Erreur d'un formulaire de compte : texte propre aux comptes s'il existe, sinon le texte commun. */
export function accountError(error: Error, t: TFunction): string {
  const key = `auth.errors.${error instanceof ApiError ? error.code : ''}`
  return i18n.exists(key) ? t(key as 'auth.errors.invalid_credentials') : errorText(error, t)
}
