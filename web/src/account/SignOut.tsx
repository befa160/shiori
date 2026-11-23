import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Loader2, LogOut } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { accountError, signOut } from '@/auth/session'
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
import { Checkbox } from '@/components/ui/checkbox'
import { SidebarMenuButton } from '@/components/ui/sidebar'
import { useOnline } from '@/lib/online'
import { useOffline } from '@/offline/downloads'
import { toastFailure } from '@/i18n/errors'

/** Déconnexion ; propose d'effacer aussi les tomes téléchargés sur cet appareil. `sidebar` : en
 * entrée de la barre latérale (desktop) plutôt qu'en bouton de la page Compte. */
export function SignOut({ sidebar = false }: { sidebar?: boolean }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const online = useOnline()
  const { downloads } = useOffline()
  const [erase, setErase] = useState(false)
  const out = useMutation({
    mutationFn: () => signOut(queryClient, erase ? downloads.map((d) => d.volumeId) : []),
    onError: toastFailure(t('account.signOutFailed'), t, accountError),
  })
  // Sans réseau, la session ne peut pas être fermée sur le serveur.
  const disabled = !online || out.isPending
  const icon = out.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <LogOut aria-hidden />
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        {sidebar ? (
          <SidebarMenuButton tooltip={t('account.signOut')}>
            <button type="button" disabled={disabled}>
              {icon}
              <span>{t('account.signOut')}</span>
            </button>
          </SidebarMenuButton>
        ) : (
          <Button variant="secondary" disabled={disabled}>
            {icon}
            {t('account.signOut')}
          </Button>
        )}
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogTitle>{t('account.signOutTitle')}</AlertDialogTitle>
        <AlertDialogDescription>{t('account.signOutDescription')}</AlertDialogDescription>
        {downloads.length > 0 && (
          <div className="flex items-center gap-3">
            <Checkbox id={`${id}-erase`} checked={erase} onCheckedChange={(v) => setErase(v === true)} />
            <label htmlFor={`${id}-erase`} className="flex min-h-11 items-center text-sm">
              {t('account.eraseDownloads', { count: downloads.length })}
            </label>
          </div>
        )}
        <AlertDialogFooter>
          <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={() => out.mutate()}>{t('account.signOut')}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
