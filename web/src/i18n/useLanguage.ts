import { useEffect } from 'react'
import { toast } from 'sonner'
import { type Scope, usePreference } from '@/account/preferences'
import i18n, { type Language, browserLanguage } from '.'
import { errorText } from './errors'

/** Langue du compte (sinon celle de l'instance, sinon celle du navigateur), appliquée à
 * l'interface ; `scope` dit laquelle `choose` enregistre. */
export function useLanguage(scope: Scope = 'account') {
  const { value, ready, save } = usePreference('language', 'account')
  const instance = usePreference('language', 'instance')
  useEffect(() => {
    if (ready) void i18n.changeLanguage(value ?? browserLanguage())
  }, [ready, value])
  const target = scope === 'instance' ? instance.save : save

  return {
    /** Valeur montrée par le sélecteur ; sans choix, la langue affichée. */
    value: (scope === 'instance' ? instance.value : value) ?? i18n.language,
    choose: (language: Language) => {
      // Pour le compte, la langue change tout de suite ; un échec la remet comme avant.
      const previous = i18n.language
      if (scope === 'account') void i18n.changeLanguage(language)
      target.mutate(language, {
        onError: async (error) => {
          if (scope === 'account') await i18n.changeLanguage(previous)
          // `i18n.t` et non le `t` du rendu : celui-ci parle encore la langue qui vient d'échouer.
          toast.error(i18n.t('layout.languageNotSaved'), { description: errorText(error, i18n.t) })
        },
      })
    },
  }
}
