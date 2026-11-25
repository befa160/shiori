import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useVolume } from '@/api/queries'
import type { VolumeDetail } from '@/api/types'
import { Button } from '@/components/ui/button'
import { DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { errorText } from '@/i18n/errors'
import { QueryState } from './Parts'
import { usePatchVolume } from './series'
import { Locked } from './SeriesMetadata'

/** Fenêtre « Modifier » d'un tome : son titre, et sa couverture choisie dans le lecteur à rendre. */
export function EditVolumeDialog({ volumeId, label, onDone }: { volumeId: string; label: string; onDone: () => void }) {
  const { t } = useTranslation()
  const volume = useVolume(volumeId)
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('series.editTitle', { name: label })}</DialogTitle>
        <DialogDescription>{t('volume.editDescription')}</DialogDescription>
      </DialogHeader>
      <QueryState query={volume} isEmpty={() => false} empty={null}>
        {(v) => <EditVolumeForm volume={v} onDone={onDone} />}
      </QueryState>
    </DialogContent>
  )
}

function EditVolumeForm({ volume, onDone }: { volume: VolumeDetail; onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const save = usePatchVolume(volume.id)
  const unlock = usePatchVolume(volume.id)
  const initial = volume.metadata.title ?? ''
  const [title, setTitle] = useState(initial)
  const [error, setError] = useState<string | null>(null)
  const restore = (field: 'title' | 'cover_page_id') =>
    unlock.mutate(
      { unlock: [field] },
      {
        onSuccess: (detail) => {
          if (field === 'title') setTitle(detail.metadata.title ?? '')
          else toast.success(t('volume.coverRestored'))
        },
        onError: (e) => setError(errorText(e, t)),
      },
    )
  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    if (title === initial) return onDone()
    setError(null)
    save.mutate(
      { title: title.trim() || null },
      {
        onSuccess: () => {
          toast.success(t('series.saved'))
          onDone()
        },
        onError: (e) => setError(errorText(e, t)),
      },
    )
  }
  const lockedTitle = volume.locked.includes('title')
  return (
    <form onSubmit={submit} className="min-w-0 space-y-4" noValidate>
      <div className="space-y-1.5">
        <div className="flex min-h-11 flex-wrap items-center justify-between gap-x-3">
          <label htmlFor={`${id}-title`} className="text-sm font-medium">
            {t('series.fields.title')}
          </label>
          {lockedTitle && (
            <Locked label={t('series.fields.title')} disabled={unlock.isPending} onUnlock={() => restore('title')} />
          )}
        </div>
        <Input id={`${id}-title`} aria-describedby={`${id}-title-help`} value={title} onChange={(e) => setTitle(e.target.value)} />
        <p id={`${id}-title-help`} className="text-sm text-muted-foreground">
          {t('volume.titleHelp')}
        </p>
      </div>
      <div className="space-y-1.5">
        <div className="flex min-h-11 flex-wrap items-center justify-between gap-x-3">
          <span className="text-sm font-medium">{t('series.fields.cover_page_id')}</span>
          {volume.locked.includes('cover_page_id') && (
            <Locked label={t('series.fields.cover_page_id')} disabled={unlock.isPending} onUnlock={() => restore('cover_page_id')} />
          )}
        </div>
        <p className="text-sm text-muted-foreground">{t('volume.coverHelp')}</p>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {t('series.notSaved')} · {error}
        </p>
      )}
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" onClick={onDone}>
          {t('series.cancel')}
        </Button>
        <Button type="submit" disabled={save.isPending}>
          {t('series.save')}
        </Button>
      </div>
    </form>
  )
}
