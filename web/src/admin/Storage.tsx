import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Download, Loader2, RotateCcw, Save, Trash2, Upload } from 'lucide-react'
import { type ReactNode, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { BackupFile } from '@/api/types'
import { signedOut } from '@/auth/session'
import { formatDate, formatSize } from '@/admin/format'
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
import { errorText } from '@/i18n/errors'

const KEY = ['backups']

/** Où sont la base, le cache et les sauvegardes ; sauvegarder, télécharger, restaurer. */
export function AdminStorage() {
  const { t } = useTranslation()
  const storage = useQuery({ queryKey: ['storage'], queryFn: ({ signal }) => api.storage({ signal }) })
  const backups = useQuery({ queryKey: KEY, queryFn: ({ signal }) => api.backups({ signal }) })
  if (!storage.data) return null
  const paths = { database: storage.data.database, cache: storage.data.cache, backupDir: backups.data?.dir }
  return (
    <section aria-labelledby="admin-storage" className="max-w-prose space-y-3 rounded-md bg-card p-4">
      <h2 id="admin-storage" className="text-lg font-semibold">
        {t('admin.storage')}
      </h2>
      <p className="text-sm text-muted-foreground">{t('admin.storageHelp')}</p>
      <dl className="space-y-2">
        {(['database', 'cache', 'backupDir'] as const).map(
          (key) =>
            paths[key] && (
              <div key={key}>
                <dt className="text-sm text-muted-foreground">{t(`admin.${key}`)}</dt>
                {/* Un clic sélectionne tout le chemin, prêt à copier. */}
                <dd className="font-mono text-sm break-all select-all">{paths[key]}</dd>
              </div>
            ),
        )}
      </dl>
      {backups.data && (
        <>
          <p className="text-sm text-muted-foreground">
            {backups.data.every_hours
              ? t('backups.schedule', { count: backups.data.every_hours, keep: backups.data.keep })
              : t('backups.noSchedule')}
          </p>
          <Actions />
          {backups.data.files.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('backups.none')}</p>
          ) : (
            <ul aria-label={t('backups.list')} className="divide-y divide-border">
              {backups.data.files.map((file) => (
                <BackupRow key={file.name} file={file} />
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  )
}

/** Restaurée : toutes les sessions sont fermées, on revient à la connexion. */
function useRestored() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  return {
    onSuccess: () => {
      toast.success(t('backups.restored'))
      signedOut(queryClient)
    },
    onError: (error: Error) => toast.error(t('backups.restoreFailed'), { description: errorText(error, t) }),
  }
}

function Actions() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const input = useRef<HTMLInputElement>(null)
  const [file, setFile] = useState<File | null>(null)
  const create = useMutation({
    mutationFn: api.createBackup,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: KEY }),
    onError: (error) => toast.error(t('backups.createFailed'), { description: errorText(error, t) }),
  })
  const restored = useRestored()
  const upload = useMutation({
    mutationFn: async (chosen: File) => api.restoreBackup((await api.uploadBackup(chosen)).name),
    onSuccess: restored.onSuccess,
    onError: (error) => {
      // Un fichier téléversé reste sur le serveur même si sa restauration échoue : la liste le montre.
      void queryClient.invalidateQueries({ queryKey: KEY })
      restored.onError(error)
    },
  })
  return (
    <div className="flex flex-wrap gap-2">
      <Button variant="secondary" disabled={create.isPending} onClick={() => create.mutate()}>
        {create.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <Save aria-hidden />}
        {t('backups.create')}
      </Button>
      <Button variant="secondary" disabled={upload.isPending} onClick={() => input.current?.click()}>
        {upload.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <Upload aria-hidden />}
        {t('backups.fromFile')}
      </Button>
      <input
        ref={input}
        type="file"
        accept=".db,application/vnd.sqlite3"
        aria-label={t('backups.fromFile')}
        className="hidden"
        onChange={(e) => {
          setFile(e.target.files?.[0] ?? null)
          e.target.value = ''
        }}
      />
      <AlertDialog open={file !== null} onOpenChange={(open) => !open && setFile(null)}>
        <AlertDialogContent>
          <AlertDialogTitle className="break-all">{t('backups.restoreTitle', { name: file?.name })}</AlertDialogTitle>
          <AlertDialogDescription>{t('backups.restoreDescription')}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => file && upload.mutate(file)}>{t('backups.restore')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

function BackupRow({ file }: { file: BackupFile }) {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const restore = useMutation({ mutationFn: () => api.restoreBackup(file.name), ...useRestored() })
  const remove = useMutation({
    mutationFn: () => api.deleteBackup(file.name),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: KEY }),
    onError: (error) => toast.error(t('backups.deleteFailed'), { description: errorText(error, t) }),
  })
  const busy = restore.isPending || remove.isPending
  return (
    <li className="flex items-center gap-1 py-3">
      <div className="min-w-0 flex-1">
        <p title={file.name} className="truncate font-mono text-sm">
          {file.name}
        </p>
        <p className="text-xs text-muted-foreground">
          {formatDate(new Date(file.modified_ms).toISOString(), i18n.language)} · {formatSize(file.size, i18n.language)}
        </p>
      </div>
      <Button asChild variant="ghost" size="icon">
        <a href={`/api/admin/backups/${encodeURIComponent(file.name)}`} download aria-label={t('backups.download', { name: file.name })}>
          <Download aria-hidden />
        </a>
      </Button>
      <Confirm
        icon={restore.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <RotateCcw aria-hidden />}
        label={t('backups.restoreNamed', { name: file.name })}
        title={t('backups.restoreTitle', { name: file.name })}
        description={t('backups.restoreDescription')}
        action={t('backups.restore')}
        disabled={busy}
        onConfirm={() => restore.mutate()}
      />
      <Confirm
        icon={<Trash2 aria-hidden />}
        label={t('backups.deleteNamed', { name: file.name })}
        title={t('backups.deleteTitle', { name: file.name })}
        description={t('backups.deleteDescription')}
        action={t('backups.delete')}
        disabled={busy}
        destructive
        onConfirm={() => remove.mutate()}
      />
    </li>
  )
}

/** Bouton icône qui demande confirmation avant d'agir. */
function Confirm(props: {
  icon: ReactNode
  label: string
  title: string
  description: string
  action: string
  disabled: boolean
  destructive?: boolean
  onConfirm: () => void
}) {
  const { t } = useTranslation()
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          disabled={props.disabled}
          aria-label={props.label}
          className={props.destructive ? 'text-destructive' : undefined}
        >
          {props.icon}
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogTitle className="break-all">{props.title}</AlertDialogTitle>
        <AlertDialogDescription>{props.description}</AlertDialogDescription>
        <AlertDialogFooter>
          <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={props.onConfirm}>{props.action}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
