import type { UseQueryResult } from '@tanstack/react-query'
import { ChevronLeft } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useParams } from 'react-router'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { errorText } from '@/i18n/errors'
import { NotFound } from '@/pages/NotFound'

/** Monte l'écran seulement si `:id` est un entier positif ; sinon « Page introuvable », au lieu
 * d'envoyer `NaN` à l'API et d'afficher son message technique. */
export function WithId({ children }: { children: (id: number) => ReactNode }) {
  const id = Number(useParams().id)
  return Number.isInteger(id) && id > 0 ? children(id) : <NotFound />
}

/** Bouton « Retour » explicite vers l'écran parent (la barre du navigateur est masquée en PWA). */
export function BackLink({ to, label }: { to: string; label: string }) {
  return (
    <Button asChild variant="ghost" className="-ml-3 max-w-[calc(100%+0.75rem)] text-muted-foreground">
      <Link to={to} title={label}>
        <ChevronLeft aria-hidden />
        {/* Un nom de bibliothèque très long se tronque au lieu d'élargir la page. */}
        <span className="truncate">{label}</span>
      </Link>
    </Button>
  )
}

/** Chargement, erreur ou liste vide ; sinon le contenu. */
export function QueryState<T>({
  query,
  empty,
  isEmpty,
  notFound,
  loading,
  children,
}: {
  query: UseQueryResult<T>
  empty: ReactNode
  /** Squelettes à la forme du contenu ; des lignes par défaut. */
  loading?: ReactNode
  /** Message à la place du « introuvable » générique du serveur. */
  notFound?: string
  isEmpty: (data: T) => boolean
  children: (data: T) => ReactNode
}) {
  const { t } = useTranslation()
  if (query.isPending) {
    if (loading) return loading
    return (
      <div aria-busy="true" className="space-y-2">
        <span className="sr-only">{t('common.loading')}</span>
        {[0, 1, 2].map((i) => (
          <Skeleton key={i} className="h-14 bg-card" />
        ))}
      </div>
    )
  }
  if (query.isError) {
    return (
      <div role="alert" className="space-y-3 rounded-md bg-card p-4">
        <p className="text-destructive first-letter:uppercase">{errorText(query.error, t, notFound)}</p>
        {/* Pendant le nouvel essai, la requête repasse en chargement : les squelettes le montrent. */}
        <Button variant="secondary" onClick={() => query.refetch()}>
          {t('common.retry')}
        </Button>
      </div>
    )
  }
  if (isEmpty(query.data)) {
    return <div className="rounded-md bg-card p-4 text-muted-foreground">{empty}</div>
  }
  return children(query.data)
}
