import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { HardDriveDownload, Loader2, LogOut, Settings } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { Device } from '@/api/types'
import { formatDate } from '@/admin/format'
import { PageTurnSetting } from '@/admin/PageTurnSetting'
import { ThemeSetting } from '@/admin/ThemeSetting'
import { accountError, signOut, useUser } from '@/auth/session'
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
import { Checkbox } from '@/components/ui/checkbox'
import { useOnline } from '@/lib/online'
import { useOffline } from '@/offline/downloads'
import { LanguageSelect } from '@/pages/LanguageSelect'
import { deviceName } from './device'
import { PasswordForm } from './PasswordForm'
import { TotpSection } from './TotpSection'

/** Mon compte : préférences, mot de passe, double authentification, appareils connectés, déconnexion ; raccourcis vers
 * les tomes téléchargés et, pour l'administrateur, l'administration. */
export function AccountPage() {
  const { t } = useTranslation()
  const user = useUser()
  const queryClient = useQueryClient()
  return (
    <section className="space-y-8">
      <title>{`${t('account.title')} · Shiori`}</title>
      <div className="space-y-1">
        <h1 className="text-2xl font-semibold">{t('account.title')}</h1>
        {user && (
          <p className="text-muted-foreground">
            <span className="font-medium break-all text-foreground">{user.username}</span> · {t(`account.roles.${user.role}`)}
          </p>
        )}
      </div>
      <div className="flex flex-wrap gap-2">
        {user?.role === 'admin' && (
          <Button asChild variant="secondary">
            <Link to="/admin">
              <Settings aria-hidden />
              {t('nav.admin')}
            </Link>
          </Button>
        )}
        <Button asChild variant="secondary">
          <Link to="/downloads">
            <HardDriveDownload aria-hidden />
            {t('nav.downloads')}
          </Link>
        </Button>
        <SignOut />
      </div>
      <section aria-labelledby="account-preferences" className="max-w-prose space-y-6 rounded-md bg-card p-4">
        <h2 id="account-preferences" className="text-lg font-semibold">
          {t('account.preferences')}
        </h2>
        <div className="space-y-1">
          <p className="text-sm font-medium">{t('admin.language')}</p>
          <LanguageSelect />
        </div>
        <div className="space-y-1">
          <p className="text-sm font-medium">{t('admin.theme')}</p>
          <ThemeSetting />
        </div>
        <PageTurnSetting />
      </section>
      <section aria-labelledby="account-password" className="max-w-prose space-y-3 rounded-md bg-card p-4">
        <h2 id="account-password" className="text-lg font-semibold">
          {t('auth.changePassword')}
        </h2>
        <p className="text-sm text-muted-foreground">{t('account.passwordHelp')}</p>
        <PasswordForm
          onDone={() => {
            toast.success(t('account.passwordChanged'))
            void queryClient.invalidateQueries({ queryKey: ['devices'] })
          }}
        />
      </section>
      <TotpSection />
      <Devices />
    </section>
  )
}

function Devices() {
  const { t } = useTranslation()
  const devices = useQuery({ queryKey: ['devices'], queryFn: ({ signal }) => api.devices({ signal }) })
  return (
    <section aria-labelledby="account-devices" className="max-w-prose space-y-3 rounded-md bg-card p-4">
      <h2 id="account-devices" className="text-lg font-semibold">
        {t('account.devices')}
      </h2>
      <p className="text-sm text-muted-foreground">{t('account.devicesHelp')}</p>
      <QueryState query={devices} empty={null} isEmpty={(list) => list.length === 0}>
        {(list) => (
          <ul className="divide-y divide-border">
            {list.map((device) => (
              <DeviceRow key={device.id} device={device} />
            ))}
          </ul>
        )}
      </QueryState>
    </section>
  )
}

function DeviceRow({ device }: { device: Device }) {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const revoke = useMutation({
    mutationFn: () => api.revokeDevice(device.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['devices'] }),
    onError: (error) => toast.error(t('account.revokeFailed'), { description: accountError(error, t) }),
  })
  return (
    <li className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        {/* Le texte complet en infobulle : distinguer deux appareils du même modèle. */}
        <p title={device.label} className="truncate text-sm">
          {deviceName(device.label)}
        </p>
        <p className="text-xs text-muted-foreground">
          {device.current ? t('account.thisDevice') : t('account.lastSeen', { date: formatDate(device.last_seen_at, i18n.language) })}
        </p>
      </div>
      {!device.current && (
        <Button variant="secondary" disabled={revoke.isPending} onClick={() => revoke.mutate()}>
          {t('account.revoke')}
        </Button>
      )}
    </li>
  )
}

/** Déconnexion ; propose d'effacer aussi les tomes téléchargés sur cet appareil. */
function SignOut() {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const online = useOnline()
  const { downloads } = useOffline()
  const [erase, setErase] = useState(false)
  const out = useMutation({
    mutationFn: () => signOut(queryClient, erase ? downloads.map((d) => d.volumeId) : []),
    onError: (error) => toast.error(t('account.signOutFailed'), { description: accountError(error, t) }),
  })
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        {/* Sans réseau, la session ne peut pas être fermée sur le serveur. */}
        <Button variant="secondary" disabled={!online || out.isPending}>
          {out.isPending ? <Loader2 aria-hidden className="animate-spin" /> : <LogOut aria-hidden />}
          {t('account.signOut')}
        </Button>
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
