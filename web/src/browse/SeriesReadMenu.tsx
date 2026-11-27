import { useQueryClient } from '@tanstack/react-query'
import { EllipsisVertical } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api } from '@/api/client'
import type { ReadingStatus } from '@/api/types'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { toastFailure } from '@/i18n/errors'
import { useSeriesAction } from './series'

/** « Marquer comme lu / non lu » pour toute une série, pour le compte connecté (lot 10.3), sur sa
 * carte (`card` : pastille sur la couverture) et sur sa page. « Non lu » efface la progression de
 * tous les tomes : il demande confirmation. */
export function SeriesReadMenu({
  id,
  name,
  status,
  volumes,
  card = false,
  extra,
}: {
  id: number
  name: string
  status: ReadingStatus
  volumes: number
  card?: boolean
  /** Entrées de plus, en fin de menu (page de la série). */
  extra?: ReactNode
}) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [confirm, setConfirm] = useState(false)
  const mark = useSeriesAction(id, (read: boolean) => api.markSeries(id, read))
  const run = (read: boolean) =>
    mark.mutate(read, {
      // La progression de chaque tome, gardée à part pour le lecteur, est aussi à relire.
      onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['volumes'] }),
      onError: toastFailure(t('series.markSeriesFailed'), t),
    })
  const label = t('series.actions', { label: name })
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          {card ? (
            // Pastille de 36 px, zone de toucher de 44 px, comme le menu d'un tome.
            <button
              type="button"
              aria-label={label}
              className="relative flex size-9 items-center justify-center rounded-full bg-black/70 text-white transition-colors after:absolute after:-inset-1 hover:bg-black/90"
            >
              <EllipsisVertical aria-hidden className="size-5" />
            </button>
          ) : (
            <Button size="icon-lg" variant="secondary" aria-label={label}>
              <EllipsisVertical aria-hidden />
            </Button>
          )}
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {status !== 'read' && <DropdownMenuItem onSelect={() => run(true)}>{t('series.markRead')}</DropdownMenuItem>}
          {status !== 'unread' && <DropdownMenuItem onSelect={() => setConfirm(true)}>{t('series.markUnread')}</DropdownMenuItem>}
          {extra}
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogTitle>{t('series.unreadTitle', { name })}</AlertDialogTitle>
          <AlertDialogDescription>{t('series.unreadDescription', { count: volumes })}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('series.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => run(false)}>{t('series.markUnread')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
