import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { api } from '@/api/client'

/** Où sont la base et le cache sur le serveur (lecture seule) : pour sauvegarder, ou retrouver
 * les données d'une installation Windows ou macOS. */
export function AdminStorage() {
  const { t } = useTranslation()
  const storage = useQuery({ queryKey: ['storage'], queryFn: ({ signal }) => api.storage({ signal }) })
  if (!storage.data) return null
  return (
    <section aria-labelledby="admin-storage" className="max-w-prose space-y-3 rounded-md bg-card p-4">
      <h2 id="admin-storage" className="text-lg font-semibold">
        {t('admin.storage')}
      </h2>
      <p className="text-sm text-muted-foreground">{t('admin.storageHelp')}</p>
      <dl className="space-y-2">
        {(['database', 'cache'] as const).map((key) => (
          <div key={key}>
            <dt className="text-sm text-muted-foreground">{t(`admin.${key}`)}</dt>
            {/* Un clic sélectionne tout le chemin, prêt à copier. */}
            <dd className="font-mono text-sm break-all select-all">{storage.data[key]}</dd>
          </div>
        ))}
      </dl>
    </section>
  )
}
