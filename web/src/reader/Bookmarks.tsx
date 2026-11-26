// Marque-pages du lecteur (lot 1.2c) : marquer la page affichée, et la liste du tome pour y
// revenir, annoter ou retirer. Hors ligne, en lecture seule (Q71).
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Bookmark as BookmarkIcon, BookmarkCheck, BookmarkPlus, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useVolumeBookmarks } from '@/api/queries'
import type { Bookmark } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { toastFailure } from '@/i18n/errors'
import { useOnline } from '@/lib/online'

/** Changements des marque-pages : chaque réussite redemande toutes les listes (`['bookmarks']`). */
function useBookmarkActions() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const options = {
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['bookmarks'] }),
    onError: toastFailure(t('reader.bookmarkFailed'), t),
  }
  return {
    add: useMutation({ mutationFn: (v: { volumeId: string; pageId: string; note?: string }) => api.addBookmark(v.volumeId, v.pageId, v.note), ...options }),
    note: useMutation({ mutationFn: (v: { id: number; note: string }) => api.setBookmarkNote(v.id, v.note), ...options }),
    remove: useMutation({ mutationFn: (b: Bookmark) => api.deleteBookmark(b.id), ...options }),
  }
}

/** Bouton qui marque (ou démarque) la page affichée, et panneau des marque-pages du tome. */
export function BookmarkControls({ volumeId, pageId, onGo }: { volumeId: string; pageId: string; onGo: (ordinal: number) => void }) {
  const { t } = useTranslation()
  const online = useOnline()
  const list = useVolumeBookmarks(volumeId).data ?? []
  const current = list.find((b) => b.page_id === pageId)
  const { add, note, remove } = useBookmarkActions()
  const toggle = () => {
    if (!current) return add.mutate({ volumeId, pageId }, { onSuccess: () => toast.success(t('reader.bookmarkAdded')) })
    remove.mutate(current, {
      onSuccess: () =>
        toast.success(t('reader.bookmarkRemoved'), {
          // La note partirait avec le marque-page : « Annuler » le remet tel quel.
          action: { label: t('reader.undo'), onClick: () => add.mutate({ volumeId, pageId: current.page_id, note: current.note }) },
        }),
    })
  }
  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        aria-label={t(current ? 'reader.unbookmark' : 'reader.bookmark')}
        aria-pressed={current !== undefined}
        disabled={!online || add.isPending || remove.isPending}
        onClick={toggle}
      >
        {current ? <BookmarkCheck aria-hidden className="text-primary" /> : <BookmarkPlus aria-hidden />}
      </Button>
      <Popover>
        <PopoverTrigger asChild>
          <Button variant="ghost" size="icon" aria-label={t('reader.bookmarks', { count: list.length })}>
            <BookmarkIcon aria-hidden />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="end" className="max-h-[70dvh] w-80 space-y-3 overflow-y-auto">
          <p className="text-sm text-muted-foreground">{t('reader.bookmarksTitle')}</p>
          {!online && <p className="text-sm text-muted-foreground">{t('reader.bookmarksOffline')}</p>}
          {list.length === 0 ? (
            <p className="text-sm">{t('reader.noBookmarks')}</p>
          ) : (
            <ul className="space-y-3">
              {list.map((b) => (
                <li key={b.id} className="space-y-1">
                  <div className="flex items-center justify-between gap-2">
                    <Button variant="ghost" className="-ml-3 h-11" onClick={() => onGo(b.ordinal)}>
                      {t('reader.bookmarkPage', { page: b.ordinal + 1 })}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t('reader.removeBookmark', { page: b.ordinal + 1 })}
                      disabled={!online}
                      onClick={() => remove.mutate(b)}
                    >
                      <Trash2 aria-hidden />
                    </Button>
                  </div>
                  <Input
                    // Une clé par note enregistrée : la valeur repart de celle du serveur.
                    key={b.note}
                    defaultValue={b.note}
                    maxLength={500}
                    placeholder={t('reader.notePlaceholder')}
                    aria-label={t('reader.bookmarkNote', { page: b.ordinal + 1 })}
                    disabled={!online}
                    onBlur={(e) => {
                      const value = e.target.value.trim()
                      if (value !== b.note) note.mutate({ id: b.id, note: value })
                    }}
                    onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
                  />
                </li>
              ))}
            </ul>
          )}
        </PopoverContent>
      </Popover>
    </>
  )
}
