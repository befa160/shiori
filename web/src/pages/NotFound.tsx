import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Button } from '@/components/ui/button'

export function NotFound() {
  const { t } = useTranslation()
  return (
    <section className="space-y-3">
      <h1 className="text-xl font-semibold">{t('notFound.title')}</h1>
      <Button asChild>
        <Link to="/">{t('notFound.home')}</Link>
      </Button>
    </section>
  )
}

/** Écran qui n'a pas pu s'afficher : son code introuvable (Shiori mis à jour pendant que
 * l'onglet était ouvert, serveur coupé) ou une erreur pendant son affichage. Le texte ne
 * présume pas de la cause ; recharger règle le cas le plus courant, la mise à jour. */
export function LoadFailed() {
  const { t } = useTranslation()
  return (
    <main className="min-h-dvh space-y-3 bg-background p-4 text-foreground">
      <h1 className="text-xl font-semibold">{t('loadFailed.title')}</h1>
      <p className="text-muted-foreground">{t('loadFailed.hint')}</p>
      <Button onClick={() => window.location.reload()}>{t('loadFailed.reload')}</Button>
    </main>
  )
}
