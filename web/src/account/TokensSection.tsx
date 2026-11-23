import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Loader2, Plus, Trash2 } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ApiError, api } from '@/api/client'
import type { ApiToken } from '@/api/types'
import { formatDate } from '@/admin/format'
import { describedBy } from '@/auth/describedBy'
import { Field } from '@/auth/Field'
import { PasswordInput } from '@/auth/PasswordInput'
import { accountError } from '@/auth/session'
import { QueryState } from '@/browse/Parts'
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
import { toastFailure } from '@/i18n/errors'

const KEY = ['tokens']
/** Durées proposées, en jours ; `never` : sans expiration. */
const DURATIONS = ['never', '30', '90', '365'] as const
/** Erreurs qui portent sur le mot de passe ; les autres, sur le nom. */
const PASSWORD_ERRORS = ['wrong_password', 'rate_limited']

/** Jetons d'API du compte : pour un autre logiciel (liseuse, OPDS), lecture et progression seulement. */
export function TokensSection() {
  const { t } = useTranslation()
  const tokens = useQuery({ queryKey: KEY, queryFn: ({ signal }) => api.tokens({ signal }) })
  const [creating, setCreating] = useState(false)
  return (
    <section aria-labelledby="account-tokens" className="max-w-prose space-y-3 rounded-md bg-card p-4">
      <h2 id="account-tokens" className="text-lg font-semibold">
        {t('tokens.title')}
      </h2>
      <p className="text-sm text-muted-foreground">{t('tokens.help')}</p>
      <QueryState query={tokens} empty={null} isEmpty={(list) => list.length === 0}>
        {(list) => (
          <ul className="divide-y divide-border">
            {list.map((token) => (
              <TokenRow key={token.id} token={token} now={new Date(tokens.dataUpdatedAt).toISOString()} />
            ))}
          </ul>
        )}
      </QueryState>
      <Button variant="secondary" onClick={() => setCreating(true)}>
        <Plus aria-hidden />
        {t('tokens.new')}
      </Button>
      <Dialog open={creating} onOpenChange={setCreating}>
        {creating && <NewToken onDone={() => setCreating(false)} />}
      </Dialog>
    </section>
  )
}

/** `now` : l'heure de réception de la liste, pour dire si un jeton est expiré sans lire l'horloge
 * pendant le rendu. */
function TokenRow({ token, now }: { token: ApiToken; now: string }) {
  const { t, i18n } = useTranslation()
  const queryClient = useQueryClient()
  const revoke = useMutation({
    mutationFn: () => api.revokeToken(token.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: KEY }),
    onError: toastFailure(t('tokens.revokeFailed'), t, accountError),
  })
  const date = (iso: string) => formatDate(iso, i18n.language)
  const expired = token.expires_at !== null && token.expires_at < now
  const details = [
    token.last_used_at ? t('tokens.lastUsed', { date: date(token.last_used_at) }) : t('tokens.neverUsed'),
    token.expires_at ? t(expired ? 'tokens.expired' : 'tokens.expires', { date: date(token.expires_at) }) : t('tokens.noExpiry'),
  ]
  return (
    <li className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        <p title={token.name} className="truncate text-sm font-medium">
          {token.name}
        </p>
        <p className="font-mono text-xs text-muted-foreground">{token.prefix}…</p>
        <p className={expired ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}>{details.join(' · ')}</p>
      </div>
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="ghost" size="icon" disabled={revoke.isPending} aria-label={t('tokens.revokeNamed', { name: token.name })} className="text-destructive">
            <Trash2 aria-hidden />
          </Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogTitle>{t('tokens.revokeTitle', { name: token.name })}</AlertDialogTitle>
          <AlertDialogDescription>{t('tokens.revokeDescription')}</AlertDialogDescription>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('admin.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => revoke.mutate()}>{t('tokens.revoke')}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </li>
  )
}

function NewToken({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [name, setName] = useState('')
  const [duration, setDuration] = useState<(typeof DURATIONS)[number]>('never')
  const [password, setPassword] = useState('')
  const create = useMutation({
    mutationFn: () => api.createToken({ name: name.trim(), ...(duration === 'never' ? {} : { expires_in_days: Number(duration) }), password }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: KEY }),
  })
  const submit = (e: FormEvent) => {
    e.preventDefault()
    create.mutate()
  }
  const error = create.error ? accountError(create.error, t) : null
  const onPassword = create.error instanceof ApiError && PASSWORD_ERRORS.includes(create.error.code)
  const nameError = onPassword ? null : error
  const passwordError = onPassword ? error : null
  if (create.data) {
    const token = create.data.token
    const copy = async () => {
      try {
        await navigator.clipboard.writeText(token)
        toast.success(t('tokens.copied'))
      } catch {
        toast.error(t('tokens.copyFailed'))
      }
    }
    return (
      <DialogContent closeLabel={t('common.close')} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle className="break-all">{create.data.name}</DialogTitle>
          <DialogDescription>{t('tokens.once')}</DialogDescription>
        </DialogHeader>
        <div className="flex items-center gap-2 rounded-md bg-muted p-3">
          <code className="flex-1 font-mono text-sm break-all select-all">{token}</code>
          <Button variant="ghost" size="icon" aria-label={t('tokens.copy')} onClick={() => void copy()}>
            <Copy aria-hidden />
          </Button>
        </div>
        <div className="flex justify-end">
          <Button onClick={onDone}>{t('accounts.noted')}</Button>
        </div>
      </DialogContent>
    )
  }
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('tokens.new')}</DialogTitle>
        <DialogDescription>{t('tokens.newHelp')}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit} className="space-y-4" noValidate>
        <Field id={`${id}-name`} label={t('tokens.name')} help={t('tokens.nameHelp')} error={nameError}>
          <Input
            id={`${id}-name`}
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={64}
            autoComplete="off"
            aria-invalid={Boolean(nameError)}
            aria-describedby={describedBy(`${id}-name`, true, Boolean(nameError))}
          />
        </Field>
        <div className="space-y-1">
          <p id={`${id}-duration`} className="text-sm font-medium">
            {t('tokens.duration')}
          </p>
          <Select value={duration} onValueChange={(v) => setDuration(v as (typeof DURATIONS)[number])}>
            <SelectTrigger aria-labelledby={`${id}-duration`} className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent position="popper" align="start">
              {DURATIONS.map((d) => (
                <SelectItem key={d} value={d}>
                  {t(`tokens.durations.${d}`)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <Field id={`${id}-password`} label={t('auth.current')} error={passwordError}>
          <PasswordInput
            id={`${id}-password`}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            aria-invalid={Boolean(passwordError)}
            aria-describedby={describedBy(`${id}-password`, false, Boolean(passwordError))}
          />
        </Field>
        <div className="flex flex-col-reverse gap-2 pt-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" onClick={onDone}>
            {t('admin.cancel')}
          </Button>
          <Button type="submit" disabled={create.isPending}>
            {create.isPending && <Loader2 aria-hidden className="animate-spin" />}
            {t('tokens.create')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}
