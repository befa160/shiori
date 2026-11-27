import type { TFunction } from 'i18next'
import { toast } from 'sonner'
import { ApiError } from '@/api/client'
import type { Issue } from '@/api/types'
import i18n from '.'

/** Texte d'une erreur dans la langue de l'interface, d'après son code stable. `notFound` précise
 * ce qui est introuvable ; un code inconnu garde le message du serveur. */
export function errorText(error: Error, t: TFunction, notFound?: string): string {
  if (!(error instanceof ApiError)) return error.message
  if (error.code === 'not_found' && notFound) return notFound
  const key = `errors.${error.code}`
  return i18n.exists(key) ? t(key as 'errors.network', { status: error.status }) : error.message
}

/** `onError` d'une action ratée : `title` en titre, l'erreur traduite par `describe` dessous. */
export const toastFailure =
  (title: string, t: TFunction, describe: (error: Error, t: TFunction) => string = errorText) =>
  (error: Error) =>
    toast.error(title, { description: describe(error, t) })

/** Texte d'un problème de scan, d'après son code stable ; un code inconnu garde le message du serveur. */
export function issueText(issue: Pick<Issue, 'message'> & { code: string }, t: TFunction): string {
  const key = `issues.${issue.code}`
  return i18n.exists(key) ? t(key as 'issues.no_pages') : issue.message
}

const SOURCE_ERRORS = ['offline', 'rate_limited', 'not_found', 'http_error', 'bad_response', 'missing_key', 'invalid_key'] as const

/** Dernier échec d'une source web, d'après son code stable ; un code inconnu reste tel quel. */
export function sourceErrorText(code: string, t: TFunction): string {
  const known = SOURCE_ERRORS.find((c) => c === code)
  return known ? t(`admin.lastError.${known}`) : code
}
