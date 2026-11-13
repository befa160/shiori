import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Loader2, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ApiError, api } from '@/api/client'
import { useLibraries } from '@/api/queries'
import type { Library, LibraryKind } from '@/api/types'
import { QueryState } from '@/browse/parts'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { errorText } from '@/i18n/errors'
import { useStartScan } from './useStartScan'

const KINDS: LibraryKind[] = ['manga', 'comics', 'webtoon']
// Ce qui montre les bibliothèques et leurs séries : redemandé après une création ou une suppression.
const LISTS = [['libraries'], ['series'], ['continue']]

/** Bibliothèques : liste, création (suivie d'un scan), scan, suppression confirmée. */
export function AdminLibraries() {
  const { t } = useTranslation()
  const libraries = useLibraries()
  const [creating, setCreating] = useState(false)
  return (
    <section aria-labelledby="admin-libraries" className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 id="admin-libraries" className="text-lg font-semibold">
          {t('admin.libraries')}
        </h2>
        <Button onClick={() => setCreating(true)}>
          <Plus aria-hidden />
          {t('admin.add')}
        </Button>
      </div>
      <QueryState query={libraries} isEmpty={(list) => list.length === 0} empty={<p>{t('admin.empty')}</p>}>
        {(list) => (
          <ul className="space-y-2">
            {list.map((library) => (
              <LibraryRow key={library.id} library={library} />
            ))}
          </ul>
        )}
      </QueryState>
      <Dialog open={creating} onOpenChange={setCreating}>
        {creating && <NewLibrary onDone={() => setCreating(false)} />}
      </Dialog>
    </section>
  )
}

function LibraryRow({ library }: { library: Library }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const scan = useStartScan()
  const remove = useMutation({
    mutationFn: () => api.deleteLibrary(library.id),
    onSuccess: () => {
      for (const queryKey of LISTS) void queryClient.invalidateQueries({ queryKey })
      toast.success(t('admin.deleted', { name: library.name }))
    },
    onError: (error) => toast.error(t('admin.deleteFailed'), { description: errorText(error, t) }),
  })
  return (
    <li className="flex flex-col gap-3 rounded-md bg-card p-4 sm:flex-row sm:items-center">
      <div className="min-w-0 flex-1">
        <h3 title={library.name} className="truncate font-medium">
          {library.name}
        </h3>
        <p className="text-sm text-muted-foreground">
          {[t(`kinds.${library.kind}`), t('counts.series', { count: library.series_count }), t('counts.volumes', { count: library.volume_count })].join(' · ')}
        </p>
        <p className="mt-1 font-mono text-xs [overflow-wrap:anywhere] text-muted-foreground">{library.root}</p>
      </div>
      <div className="flex shrink-0 gap-2">
        <Button variant="secondary" disabled={scan.isPending} onClick={() => scan.mutate({ library })} aria-label={t('admin.scanNamed', { name: library.name })}>
          {scan.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <RefreshCw aria-hidden />}
          {t('admin.scan')}
        </Button>
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="ghost" size="icon" aria-label={t('admin.deleteNamed', { name: library.name })} className="text-destructive">
              <Trash2 aria-hidden />
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogTitle>{t('admin.deleteTitle', { name: library.name })}</AlertDialogTitle>
            <AlertDialogDescription>{t('admin.deleteDescription')}</AlertDialogDescription>
            <AlertDialogFooter>
              <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
              <AlertDialogAction onClick={() => remove.mutate()}>{t('admin.delete')}</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </li>
  )
}

// Erreurs du serveur rattachées au champ qu'elles concernent ; les autres s'affichent en tête.
const FIELD_OF: Record<string, 'name' | 'root'> = {
  invalid_name: 'name',
  name_taken: 'name',
  invalid_root: 'root',
  root_overlaps: 'root',
}

function NewLibrary({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const scan = useStartScan()
  const id = useId()
  const [name, setName] = useState('')
  const [root, setRoot] = useState('')
  const [kind, setKind] = useState<LibraryKind>('manga')
  const create = useMutation({
    mutationFn: () => api.createLibrary({ name: name.trim(), root: root.trim(), kind }),
    onSuccess: (library) => {
      for (const queryKey of LISTS) void queryClient.invalidateQueries({ queryKey })
      toast.success(t('admin.created', { name: library.name }))
      // Une bibliothèque neuve est vide tant qu'elle n'a pas été scannée.
      scan.mutate({ library })
      onDone()
    },
  })
  const error = create.error
  const field = error instanceof ApiError ? FIELD_OF[error.code] : undefined
  const message = error ? errorText(error, t) : null
  const submit = (e: FormEvent) => {
    e.preventDefault()
    create.mutate()
  }
  const fieldError = (which: 'name' | 'root') =>
    field === which && (
      <p id={`${id}-${which}-error`} role="alert" className="text-sm text-destructive">
        {message}
      </p>
    )
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('admin.newTitle')}</DialogTitle>
        <DialogDescription>{t('admin.newDescription')}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit} className="space-y-4" noValidate>
        {message && !field && (
          <p role="alert" className="text-sm text-destructive">
            {message}
          </p>
        )}
        <div className="space-y-1">
          <label htmlFor={`${id}-name`} className="text-sm font-medium">
            {t('admin.name')}
          </label>
          <Input
            id={`${id}-name`}
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
            aria-invalid={field === 'name'}
            aria-describedby={field === 'name' ? `${id}-name-error` : undefined}
          />
          {fieldError('name')}
        </div>
        <div className="space-y-1">
          <label htmlFor={`${id}-root`} className="text-sm font-medium">
            {t('admin.root')}
          </label>
          <p id={`${id}-root-help`} className="text-sm text-muted-foreground">
            {t('admin.rootHelp')}
          </p>
          <Input
            id={`${id}-root`}
            value={root}
            onChange={(e) => setRoot(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            className="font-mono"
            aria-invalid={field === 'root'}
            aria-describedby={`${id}-root-help${field === 'root' ? ` ${id}-root-error` : ''}`}
          />
          {fieldError('root')}
        </div>
        <div className="space-y-1">
          <p id={`${id}-kind`} className="text-sm font-medium">
            {t('admin.kind')}
          </p>
          <p id={`${id}-kind-help`} className="text-sm text-muted-foreground">
            {t('admin.kindHelp')}
          </p>
          <Select value={kind} onValueChange={(v) => setKind(v as LibraryKind)}>
            <SelectTrigger aria-labelledby={`${id}-kind`} aria-describedby={`${id}-kind-help`} className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              {KINDS.map((k) => (
                <SelectItem key={k} value={k}>
                  {t(`kinds.${k}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col-reverse gap-2 pt-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" onClick={onDone}>
            {t('admin.cancel')}
          </Button>
          <Button type="submit" disabled={create.isPending}>
            {create.isPending && <Loader2 aria-hidden className="animate-spin" />}
            {t('admin.create')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}
