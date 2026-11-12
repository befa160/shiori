import type { TFunction } from 'i18next'
import { ApiError } from '@/api/client'
import i18n from '.'

/** Texte d'une erreur dans la langue de l'interface, d'après son code stable. `notFound` précise
 * ce qui est introuvable ; un code inconnu garde le message du serveur. */
export function errorText(error: Error, t: TFunction, notFound?: string): string {
  if (!(error instanceof ApiError)) return error.message
  if (error.code === 'not_found' && notFound) return notFound
  const key = `errors.${error.code}`
  return i18n.exists(key) ? t(key as 'errors.network', { status: error.status }) : error.message
}
