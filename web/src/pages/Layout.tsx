import { useTranslation } from 'react-i18next'
import { Link, Outlet } from 'react-router'
import { LanguageSelect } from './LanguageSelect'

/** Cadre commun hors lecteur : thème sombre, zones sûres de la PWA, choix de la langue. */
export function Layout() {
  const { t } = useTranslation()
  return (
    <div className="min-h-dvh bg-background pt-safe pr-safe pb-safe pl-safe text-foreground">
      <header className="mx-auto flex max-w-5xl items-center justify-between gap-4 px-4 pt-4">
        <Link to="/" aria-label={t('layout.home')} className="text-lg font-semibold">
          Shiori
        </Link>
        <LanguageSelect />
      </header>
      <main className="mx-auto max-w-5xl px-4 py-6">
        <Outlet />
      </main>
    </div>
  )
}
