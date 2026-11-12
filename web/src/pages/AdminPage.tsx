import { useTranslation } from 'react-i18next'
import { LanguageSelect } from './LanguageSelect'

/** Administration : la langue pour l'instant ; bibliothèques et scans aux lots 4.7 et 4.8. */
export function AdminPage() {
  const { t } = useTranslation()
  return (
    <section className="space-y-6">
      <title>{`${t('admin.title')} · Shiori`}</title>
      <h1 className="text-2xl font-semibold">{t('admin.title')}</h1>
      <section aria-labelledby="admin-language" className="max-w-prose space-y-3 rounded-md bg-card p-4">
        <h2 id="admin-language" className="text-lg font-semibold">
          {t('admin.language')}
        </h2>
        <p className="text-sm text-muted-foreground">{t('admin.languageHelp')}</p>
        <LanguageSelect />
      </section>
    </section>
  )
}
