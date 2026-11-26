import { ChevronDown } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { useSeriesBookmarks } from '@/api/queries'
import type { SeriesDetail } from '@/api/types'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { volumeLabel } from './labels'

/** Marque-pages du compte dans la série (Q70), repliés ; rien s'il n'y en a pas. */
export function SeriesBookmarks({ series }: { series: SeriesDetail }) {
  const { t, i18n } = useTranslation()
  const [open, setOpen] = useState(false)
  const listId = useId()
  const bookmarks = useSeriesBookmarks(series.id).data ?? []
  // Dans l'ordre de lecture de la série (le serveur range par identifiant de tome).
  const position = new Map(series.volumes.map((v, i) => [v.volume_id, i]))
  const sorted = bookmarks
    .filter((b) => position.has(b.volume_id))
    .sort((a, b) => position.get(a.volume_id)! - position.get(b.volume_id)! || a.ordinal - b.ordinal)
  if (sorted.length === 0) return null
  return (
    <section className="space-y-2">
      <Button id={`${listId}-toggle`} variant="ghost" className="-ml-3 h-11 text-lg font-semibold" aria-expanded={open} aria-controls={listId} onClick={() => setOpen(!open)}>
        {t('series.bookmarks', { count: sorted.length })}
        <ChevronDown aria-hidden className={cn('transition-transform', open && 'rotate-180')} />
      </Button>
      {open && (
        <ul id={listId} aria-labelledby={`${listId}-toggle`} className="max-w-prose divide-y divide-border rounded-md bg-card">
          {sorted.map((b) => {
            const volume = series.volumes[position.get(b.volume_id)!]
            return (
              <li key={b.id}>
                <Link
                  to={`/read/${b.volume_id}?${new URLSearchParams({ page: b.page_id })}`}
                  className="flex min-h-11 flex-col justify-center px-4 py-2 hover:bg-accent"
                >
                  <span>{t('series.bookmarkAt', { volume: volumeLabel(volume, t, i18n.language), page: b.ordinal + 1 })}</span>
                  {b.note && <span className="text-sm text-muted-foreground [overflow-wrap:anywhere]">{b.note}</span>}
                </Link>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}
