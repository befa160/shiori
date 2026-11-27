import type { TFunction } from 'i18next'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Link2, Loader2, Plus, Trash2 } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useLibraries } from '@/api/queries'
import type { Account, Invite, Library, Role } from '@/api/types'
import { describedBy } from '@/auth/describedBy'
import { Field } from '@/auth/Field'
import { accountError, useUser } from '@/auth/session'
import { BackLink, QueryState } from '@/browse/Parts'
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
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { toastFailure } from '@/i18n/errors'
import { formatDate } from './format'

const ROLES: Role[] = ['reader', 'admin']
const KEY = ['accounts']
const INVITES = ['invites']
/** Durées de validité d'un lien (Q78). */
const DAYS = ['1', '7', '30'] as const

/** Administration → Comptes : création (mot de passe provisoire montré une fois), rôle,
 * bibliothèques, désactivation, nouveau mot de passe, déconnexion des appareils, suppression. */
export function AccountsPage() {
  const { t } = useTranslation()
  const accounts = useQuery({ queryKey: KEY, queryFn: ({ signal }) => api.accounts({ signal }) })
  const libraries = useLibraries().data ?? []
  const [creating, setCreating] = useState(false)
  const [inviting, setInviting] = useState(false)
  const [editing, setEditing] = useState<Account | null>(null)
  return (
    <section className="space-y-6">
      <title>{`${t('accounts.title')} · Shiori`}</title>
      <BackLink to="/admin" label={t('admin.title')} />
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold">{t('accounts.title')}</h1>
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" onClick={() => setInviting(true)}>
            <Link2 aria-hidden />
            {t('invite.new')}
          </Button>
          <Button onClick={() => setCreating(true)}>
            <Plus aria-hidden />
            {t('accounts.new')}
          </Button>
        </div>
      </div>
      <QueryState query={accounts} empty={null} isEmpty={(list) => list.length === 0}>
        {(list) => (
          <ul className="space-y-2">
            {list.map((account) => (
              <li key={account.id} className="flex flex-col gap-3 rounded-md bg-card p-4 sm:flex-row sm:items-center">
                <div className="min-w-0 flex-1">
                  <h2 className="font-medium break-all">{account.username}</h2>
                  <p className="text-sm text-muted-foreground">{summary(account, libraries, t)}</p>
                </div>
                <Button variant="secondary" onClick={() => setEditing(account)} aria-label={t('accounts.editNamed', { name: account.username })}>
                  {t('accounts.edit')}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </QueryState>
      <Invites libraries={libraries} />
      <Dialog open={inviting} onOpenChange={setInviting}>
        {inviting && <NewInvite libraries={libraries} onDone={() => setInviting(false)} />}
      </Dialog>
      <Dialog open={creating} onOpenChange={setCreating}>
        {creating && <NewAccount libraries={libraries} onDone={() => setCreating(false)} />}
      </Dialog>
      <Dialog open={editing !== null} onOpenChange={(open) => !open && setEditing(null)}>
        {editing && <EditAccount account={editing} libraries={libraries} onDone={() => setEditing(null)} />}
      </Dialog>
    </section>
  )
}

/** Rôle et bibliothèques ouvertes, en mots. */
function access({ role, libraries: open }: { role: Role; libraries: number[] }, libraries: Library[], t: TFunction): string[] {
  const all = role === 'admin' || (libraries.length > 0 && libraries.every((l) => open.includes(l.id)))
  return [t(`account.roles.${role}`), all ? t('accounts.allLibraries') : t('counts.libraries', { count: open.length })]
}

function summary(account: Account, libraries: Library[], t: TFunction): string {
  const parts = access(account, libraries, t)
  if (account.totp) parts.push(t('accounts.totp'))
  if (account.disabled) parts.push(t('accounts.disabled'))
  else if (account.must_change_password) parts.push(t('accounts.provisional'))
  return parts.join(' · ')
}

/** Rôle et, pour un lecteur, ses bibliothèques. */
function Access({
  role,
  setRole,
  chosen,
  setChosen,
  libraries,
}: {
  role: Role
  setRole: (r: Role) => void
  chosen: number[]
  setChosen: (l: number[]) => void
  libraries: Library[]
}) {
  const { t } = useTranslation()
  const id = useId()
  return (
    <>
      <div className="space-y-1">
        <p id={`${id}-role`} className="text-sm font-medium">
          {t('accounts.role')}
        </p>
        <Select value={role} onValueChange={(v) => setRole(v as Role)}>
          <SelectTrigger aria-labelledby={`${id}-role`} className="w-56">
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper" align="start">
            {ROLES.map((r) => (
              <SelectItem key={r} value={r}>
                {t(`account.roles.${r}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {role === 'reader' ? (
        <fieldset className="space-y-1">
          <legend className="text-sm font-medium">{t('accounts.libraries')}</legend>
          {libraries.length === 0 && <p className="text-sm text-muted-foreground">{t('accounts.noLibraries')}</p>}
          {libraries.map((library) => (
            <div key={library.id} className="flex items-center gap-3">
              <Checkbox
                id={`${id}-${library.id}`}
                checked={chosen.includes(library.id)}
                onCheckedChange={(v) => setChosen(v === true ? [...chosen, library.id] : chosen.filter((c) => c !== library.id))}
              />
              <label htmlFor={`${id}-${library.id}`} className="flex min-h-11 min-w-0 items-center text-sm break-all">
                {library.name}
              </label>
            </div>
          ))}
        </fieldset>
      ) : (
        <p className="text-sm text-muted-foreground">{t('accounts.adminHelp')}</p>
      )}
    </>
  )
}

/** Secret montré une seule fois (mot de passe provisoire, lien d'invitation), avec Copier. */
function Revealed({ message, secret, copy, onDone }: { message: string; secret: string; copy: 'accounts' | 'invite'; onDone: () => void }) {
  const { t } = useTranslation()
  const copyIt = async () => {
    try {
      await navigator.clipboard.writeText(secret)
      toast.success(t(`${copy}.copied`))
    } catch {
      // Presse-papiers refusé (HTTP sans TLS) : le secret reste sélectionnable.
      toast.error(t(`${copy}.copyFailed`))
    }
  }
  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">{message}</p>
      <div className="flex items-center gap-2 rounded-md bg-muted p-3">
        <code className="flex-1 font-mono text-base break-all select-all">{secret}</code>
        <Button variant="ghost" size="icon" aria-label={t(`${copy}.copy`)} onClick={() => void copyIt()}>
          <Copy aria-hidden />
        </Button>
      </div>
      <div className="flex justify-end">
        <Button onClick={onDone}>{t('accounts.noted')}</Button>
      </div>
    </div>
  )
}

function NewAccount({ libraries, onDone }: { libraries: Library[]; onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [username, setUsername] = useState('')
  const [role, setRole] = useState<Role>('reader')
  // Q49 : un nouveau lecteur voit toutes les bibliothèques existantes, sauf choix contraire.
  const [chosen, setChosen] = useState(() => libraries.map((l) => l.id))
  const create = useMutation({
    mutationFn: () => api.createAccount({ username: username.trim(), role, libraries: role === 'reader' ? chosen : undefined }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: KEY }),
  })
  const submit = (e: FormEvent) => {
    e.preventDefault()
    create.mutate()
  }
  const error = create.error ? accountError(create.error, t) : null
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('accounts.new')}</DialogTitle>
        {!create.data && <DialogDescription>{t('accounts.newDescription')}</DialogDescription>}
      </DialogHeader>
      {create.data ? (
        <Revealed
          message={t('accounts.passwordOnce', { name: create.data.user.username })}
          secret={create.data.password}
          copy="accounts"
          onDone={onDone}
        />
      ) : (
        <form onSubmit={submit} className="space-y-4" noValidate>
          <Field id={`${id}-name`} label={t('auth.username')} help={t('auth.usernameHelp')} error={error}>
            <Input
              id={`${id}-name`}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              aria-invalid={Boolean(error)}
              aria-describedby={describedBy(`${id}-name`, true, Boolean(error))}
            />
          </Field>
          <Access role={role} setRole={setRole} chosen={chosen} setChosen={setChosen} libraries={libraries} />
          <div className="flex flex-col-reverse gap-2 pt-2 sm:flex-row sm:justify-end">
            <Button type="button" variant="secondary" onClick={onDone}>
              {t('admin.cancel')}
            </Button>
            <Button type="submit" disabled={create.isPending}>
              {create.isPending && <Loader2 aria-hidden className="animate-spin" />}
              {t('auth.setup')}
            </Button>
          </div>
        </form>
      )}
    </DialogContent>
  )
}

function EditAccount({ account, libraries, onDone }: { account: Account; libraries: Library[]; onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const me = useUser()
  const [role, setRole] = useState(account.role)
  const [chosen, setChosen] = useState(account.libraries)
  const [disabled, setDisabled] = useState(account.disabled)
  const refresh = () => void queryClient.invalidateQueries({ queryKey: KEY })
  const save = useMutation({
    mutationFn: () => api.updateAccount(account.id, { role, disabled, libraries: role === 'reader' ? chosen : undefined }),
    onSuccess: () => {
      refresh()
      onDone()
    },
  })
  const reset = useMutation({ mutationFn: () => api.resetPassword(account.id), onSuccess: refresh, onError: toastFailure(t('accounts.resetFailed'), t, accountError) })
  const resetTotp = useMutation({
    mutationFn: () => api.resetTotp(account.id),
    onSuccess: () => {
      refresh()
      toast.success(t('accounts.totpReset', { name: account.username }))
    },
    onError: toastFailure(t('accounts.totpResetFailed'), t, accountError),
  })
  const signOut = useMutation({
    mutationFn: () => api.signOutAccount(account.id),
    onSuccess: () => toast.success(t('accounts.signedOut', { name: account.username })),
    onError: toastFailure(t('accounts.signOutFailed'), t, accountError),
  })
  const remove = useMutation({
    mutationFn: () => api.deleteAccount(account.id),
    onSuccess: () => {
      refresh()
      toast.success(t('accounts.deleted', { name: account.username }))
      onDone()
    },
    onError: toastFailure(t('accounts.deleteFailed'), t, accountError),
  })
  const submit = (e: FormEvent) => {
    e.preventDefault()
    save.mutate()
  }
  const self = me?.id === account.id
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle className="break-all">{account.username}</DialogTitle>
        <DialogDescription>{t('accounts.editDescription')}</DialogDescription>
      </DialogHeader>
      {reset.data ? (
        <Revealed message={t('accounts.passwordOnce', { name: account.username })} secret={reset.data.password} copy="accounts" onDone={onDone} />
      ) : (
        <>
          <form onSubmit={submit} className="space-y-4" noValidate>
            {save.error && (
              <p role="alert" className="text-sm text-destructive">
                {accountError(save.error, t)}
              </p>
            )}
            <Access role={role} setRole={setRole} chosen={chosen} setChosen={setChosen} libraries={libraries} />
            {!self && (
              <div className="flex items-start gap-3">
                <Checkbox id={`${id}-disabled`} checked={disabled} onCheckedChange={(v) => setDisabled(v === true)} aria-describedby={`${id}-disabled-help`} className="mt-3" />
                <div>
                  <label htmlFor={`${id}-disabled`} className="flex min-h-11 items-center text-sm font-medium">
                    {t('accounts.disable')}
                  </label>
                  <p id={`${id}-disabled-help`} className="-mt-2 text-sm text-muted-foreground">
                    {t('accounts.disableHelp')}
                  </p>
                </div>
              </div>
            )}
            <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
              <Button type="button" variant="secondary" onClick={onDone}>
                {t('admin.cancel')}
              </Button>
              <Button type="submit" disabled={save.isPending}>
                {save.isPending && <Loader2 aria-hidden className="animate-spin" />}
                {t('accounts.save')}
              </Button>
            </div>
          </form>
          {!self && (
            <div className="space-y-2 border-t border-border pt-4">
              <Button variant="secondary" className="w-full" disabled={reset.isPending} onClick={() => reset.mutate()}>
                {t('accounts.reset')}
              </Button>
              <Button variant="secondary" className="w-full" disabled={signOut.isPending} onClick={() => signOut.mutate()}>
                {t('accounts.signOutAll')}
              </Button>
              {/* Téléphone perdu et plus de code de secours : la connexion redevient mot de passe seul. */}
              {account.totp && !resetTotp.isSuccess && (
                <Button variant="secondary" className="w-full" disabled={resetTotp.isPending} onClick={() => resetTotp.mutate()}>
                  {t('accounts.resetTotp')}
                </Button>
              )}
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button variant="ghost" className="w-full text-destructive">
                    {t('accounts.delete')}
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogTitle>{t('accounts.deleteTitle', { name: account.username })}</AlertDialogTitle>
                  <AlertDialogDescription>{t('accounts.deleteDescription')}</AlertDialogDescription>
                  <AlertDialogFooter>
                    <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
                    <AlertDialogAction onClick={() => remove.mutate()}>{t('admin.delete')}</AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          )}
        </>
      )}
    </DialogContent>
  )
}

/** Invitations en attente (périmées comprises, à révoquer) ; rien tant qu'il n'y en a pas.
 * `dataUpdatedAt` : l'heure de réception, pour dire « expirée » sans lire l'horloge au rendu. */
function Invites({ libraries }: { libraries: Library[] }) {
  const { t } = useTranslation()
  const invites = useQuery({ queryKey: INVITES, queryFn: ({ signal }) => api.invites({ signal }) })
  if (!invites.data?.length) return null
  const now = new Date(invites.dataUpdatedAt).toISOString()
  return (
    <section className="space-y-2" aria-labelledby="invites-title">
      <h2 id="invites-title" className="text-lg font-semibold">
        {t('invite.pending')}
      </h2>
      <ul className="divide-y divide-border rounded-md bg-card px-4">
        {invites.data.map((invite) => (
          <InviteRow key={invite.id} invite={invite} libraries={libraries} now={now} />
        ))}
      </ul>
    </section>
  )
}

function InviteRow({ invite, libraries, now }: { invite: Invite; libraries: Library[]; now: string }) {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const revoke = useMutation({
    mutationFn: () => api.revokeInvite(invite.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: INVITES }),
    onError: toastFailure(t('invite.revokeFailed'), t, accountError),
  })
  const expired = invite.expires_at < now
  const date = formatDate(invite.expires_at, i18n.language)
  const name = access(invite, libraries, t).join(' · ')
  return (
    <li className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium">{name}</p>
        <p className={expired ? 'text-sm text-destructive' : 'text-sm text-muted-foreground'}>
          {t(expired ? 'invite.expired' : 'invite.expires', { date })}
        </p>
      </div>
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="ghost" size="icon" disabled={revoke.isPending} aria-label={t('invite.revokeNamed', { name, date })} className="text-destructive">
            <Trash2 aria-hidden />
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogTitle>{t('invite.revokeTitle')}</AlertDialogTitle>
          <AlertDialogDescription>{t('invite.revokeDescription')}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => revoke.mutate()}>{t('invite.revoke')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </li>
  )
}

function NewInvite({ libraries, onDone }: { libraries: Library[]; onDone: () => void }) {
  const { t, i18n } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [role, setRole] = useState<Role>('reader')
  const [chosen, setChosen] = useState(() => libraries.map((l) => l.id))
  const [days, setDays] = useState<(typeof DAYS)[number]>('7')
  const create = useMutation({
    mutationFn: () => api.createInvite({ role, libraries: role === 'reader' ? chosen : undefined, days: Number(days) }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: INVITES }),
  })
  const submit = (e: FormEvent) => {
    e.preventDefault()
    create.mutate()
  }
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('invite.new')}</DialogTitle>
        {!create.data && <DialogDescription>{t('invite.newDescription')}</DialogDescription>}
      </DialogHeader>
      {create.data ? (
        <Revealed
          message={t('invite.once', { date: formatDate(create.data.expires_at, i18n.language) })}
          secret={`${window.location.origin}/invite/${create.data.token}`}
          copy="invite"
          onDone={onDone}
        />
      ) : (
        <form onSubmit={submit} className="space-y-4" noValidate>
          {create.error && (
            <p role="alert" className="text-sm text-destructive">
              {accountError(create.error, t)}
            </p>
          )}
          <Access role={role} setRole={setRole} chosen={chosen} setChosen={setChosen} libraries={libraries} />
          <div className="space-y-1">
            <p id={`${id}-days`} className="text-sm font-medium">
              {t('invite.duration')}
            </p>
            <Select value={days} onValueChange={(v) => setDays(v as (typeof DAYS)[number])}>
              <SelectTrigger aria-labelledby={`${id}-days`} className="w-56">
                <SelectValue />
              </SelectTrigger>
              <SelectContent position="popper" align="start">
                {DAYS.map((d) => (
                  <SelectItem key={d} value={d}>
                    {t(`invite.days.${d}`)}
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
              {t('invite.create')}
            </Button>
          </div>
        </form>
      )}
    </DialogContent>
  )
}
