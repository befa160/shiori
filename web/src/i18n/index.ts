import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import { en } from './en'
import { fr } from './fr'

export const LANGUAGES = ['fr', 'en'] as const
export type Language = (typeof LANGUAGES)[number]

/** Langue du navigateur parmi celles de Shiori ; anglais pour toute autre langue. */
export function browserLanguage(preferred: readonly string[] = navigator.languages): Language {
  for (const tag of preferred) {
    const found = LANGUAGES.find((l) => tag.toLowerCase().startsWith(l))
    if (found) return found
  }
  return 'en'
}

declare module 'i18next' {
  interface CustomTypeOptions {
    resources: { translation: typeof fr }
  }
}

// `<html lang>` suit la langue affichée : lecteurs d'écran et césure.
i18n.on('languageChanged', (lng) => {
  document.documentElement.lang = lng
})

// Catalogues intégrés : initialisation synchrone, le premier rendu est déjà traduit.
void i18n.use(initReactI18next).init({
  initAsync: false,
  resources: { fr: { translation: fr }, en: { translation: en } },
  lng: browserLanguage(),
  fallbackLng: 'fr',
  supportedLngs: LANGUAGES,
  interpolation: { escapeValue: false },
})

export default i18n
