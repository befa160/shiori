import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { AdminLibraries } from '@/admin/Libraries'
import { CurrentScan } from '@/admin/scans'
import { Button } from '@/components/ui/button'
import { LanguageSelect } from './LanguageSelect'

/** Administration : passe en cours, bibliothèques, langue ; l'historique des scans a sa page. */
export function AdminPage() {
  const { t } = useTranslation()
  return (
    <section className="space-y-8">
      <title>{`${t('admin.title')} · Shiori`}</title>
      <h1 className="text-2xl font-semibold">{t('admin.title')}</h1>
      <section aria-labelledby="admin-scan" className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 id="admin-scan" className="text-lg font-semibold">
            {t('scans.current')}
          </h2>
          <Button asChild variant="secondary">
            <Link to="/admin/scans">{t('scans.historyLink')}</Link>
          </Button>
        </div>
        <CurrentScan />
      </section>
      <AdminLibraries />
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
