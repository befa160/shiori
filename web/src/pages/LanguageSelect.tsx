import { Languages } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { LANGUAGES, type Language } from '@/i18n'
import { useLanguage } from '@/i18n/useLanguage'

// Chaque langue s'écrit dans sa propre langue : on la reconnaît même sans comprendre l'autre.
const NAMES: Record<Language, string> = { fr: 'Français', en: 'English' }

export function LanguageSelect() {
  const { t, i18n } = useTranslation()
  const { choose } = useLanguage()
  return (
    <Select value={i18n.language} onValueChange={(value) => choose(value as Language)}>
      <SelectTrigger aria-label={t('layout.language')} className="w-40">
        <Languages aria-hidden />
        <SelectValue />
      </SelectTrigger>
      {/* Sous le sélecteur plutôt que par-dessus : prévisible au toucher. */}
      <SelectContent position="popper" align="end">
        {LANGUAGES.map((language) => (
          <SelectItem key={language} value={language} lang={language}>
            {NAMES[language]}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
