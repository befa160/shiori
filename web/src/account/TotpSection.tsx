import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Copy, Loader2 } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ApiError, api } from '@/api/client'
import { describedBy } from '@/auth/describedBy'
import { Field } from '@/auth/Field'
import { PasswordInput } from '@/auth/PasswordInput'
import { accountError } from '@/auth/session'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { useMe } from './preferences'

/** Double authentification du compte : activation (QR, premier code, codes de secours) ou
 * désactivation (mot de passe et code). */
export function TotpSection() {
  const { t } = useTranslation()
  const me = useMe().data
  // Mode fixé à l'ouverture : l'état du compte, redemandé en cours d'activation, ne change pas la fenêtre.
  const [dialog, setDialog] = useState<'enable' | 'disable' | null>(null)
  if (!me) return null
  return (
    <section aria-labelledby="account-totp" className="max-w-prose space-y-3 rounded-md bg-card p-4">
      <h2 id="account-totp" className="text-lg font-semibold">
        {t('totp.title')}
      </h2>
      <p className="text-sm text-muted-foreground">
        {me.totp ? t('totp.enabled', { count: me.recovery_codes_left }) : t('totp.help')}
      </p>
      <Button variant={me.totp ? 'secondary' : 'default'} onClick={() => setDialog(me.totp ? 'disable' : 'enable')}>
        {t(me.totp ? 'totp.disable' : 'totp.enable')}
      </Button>
      <Dialog open={dialog !== null} onOpenChange={(open) => !open && setDialog(null)}>
        {dialog === 'enable' && <Enable onDone={() => setDialog(null)} />}
        {dialog === 'disable' && <Disable onDone={() => setDialog(null)} />}
      </Dialog>
    </section>
  )
}

/** Code saisi : 6 chiffres ou code de secours, collage permis. */
function CodeInput({ id, value, onChange, invalid }: { id: string; value: string; onChange: (v: string) => void; invalid: boolean }) {
  return (
    <Input
      id={id}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      autoComplete="one-time-code"
      autoCapitalize="none"
      spellCheck={false}
      className="font-mono tracking-widest"
      aria-invalid={invalid}
      aria-describedby={describedBy(id, true, invalid)}
    />
  )
}

function Enable({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [password, setPassword] = useState('')
  const [code, setCode] = useState('')
  const start = useMutation({ mutationFn: () => api.startTotp(password) })
  const confirm = useMutation({
    mutationFn: () => api.confirmTotp(code),
    // Tout de suite, et non à « C'est noté » : fermer par la croix laisse aussi la page juste.
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['me'] }),
  })
  const enrollment = start.data
  const codes = confirm.data?.recovery_codes
  if (!enrollment) {
    const error = start.error ? accountError(start.error, t) : null
    const submit = (e: FormEvent) => {
      e.preventDefault()
      start.mutate()
    }
    return (
      <DialogContent closeLabel={t('common.close')}>
        <DialogHeader>
          <DialogTitle>{t('totp.enableTitle')}</DialogTitle>
          <DialogDescription>{t('totp.passwordFirst')}</DialogDescription>
        </DialogHeader>
        <form onSubmit={submit} className="space-y-4" noValidate>
          <Field id={`${id}-password`} label={t('auth.current')} error={error}>
            <PasswordInput
              id={`${id}-password`}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              aria-invalid={Boolean(error)}
              aria-describedby={describedBy(`${id}-password`, false, Boolean(error))}
            />
          </Field>
          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <Button type="button" variant="secondary" onClick={onDone}>
              {t('admin.cancel')}
            </Button>
            <Button type="submit" disabled={start.isPending}>
              {start.isPending && <Loader2 aria-hidden className="animate-spin" />}
              {t('totp.continue')}
            </Button>
          </div>
        </form>
      </DialogContent>
    )
  }
  const submit = (e: FormEvent) => {
    e.preventDefault()
    confirm.mutate()
  }
  const error = confirm.error ? accountError(confirm.error, t) : null
  if (codes) {
    const copy = async () => {
      try {
        await navigator.clipboard.writeText(codes.join('\n'))
        toast.success(t('totp.copied'))
      } catch {
        toast.error(t('accounts.copyFailed'))
      }
    }
    return (
      <DialogContent closeLabel={t('common.close')} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>{t('totp.codesTitle')}</DialogTitle>
          <DialogDescription>{t('totp.codesHelp')}</DialogDescription>
        </DialogHeader>
        <ul aria-label={t('totp.codesTitle')} className="grid grid-cols-2 gap-2 rounded-md bg-muted p-3 font-mono text-sm select-all">
          {codes.map((c) => (
            <li key={c}>{c}</li>
          ))}
        </ul>
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button variant="secondary" onClick={() => void copy()}>
            <Copy aria-hidden />
            {t('totp.copy')}
          </Button>
          <Button onClick={onDone}>{t('accounts.noted')}</Button>
        </div>
      </DialogContent>
    )
  }
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('totp.enableTitle')}</DialogTitle>
        <DialogDescription>{t('totp.scan')}</DialogDescription>
      </DialogHeader>
      {/* Image et non SVG inséré : le contenu du serveur n'entre jamais dans la page. */}
      <img
        src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(enrollment.qr_svg)}`}
        alt={t('totp.qr')}
        width={200}
        height={200}
        className="mx-auto rounded-md bg-white"
      />
      <div className="space-y-1 text-sm">
        <p className="text-muted-foreground">{t('totp.manual')}</p>
        <code className="block rounded-md bg-muted p-2 font-mono break-all select-all">{enrollment.secret.match(/.{1,4}/g)?.join(' ')}</code>
        {/* Sur le téléphone même, scanner son propre écran est impossible : ouvrir l'application. */}
        <a href={enrollment.uri} className="inline-flex min-h-11 items-center font-medium underline underline-offset-4">
          {t('totp.openApp')}
        </a>
      </div>
      <form onSubmit={submit} className="space-y-4" noValidate>
        <Field id={`${id}-code`} label={t('auth.code')} help={t('totp.firstCode')} error={error}>
          <CodeInput id={`${id}-code`} value={code} onChange={setCode} invalid={Boolean(error)} />
        </Field>
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" onClick={onDone}>
            {t('admin.cancel')}
          </Button>
          <Button type="submit" disabled={confirm.isPending}>
            {confirm.isPending && <Loader2 aria-hidden className="animate-spin" />}
            {t('totp.enable')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}

const DISABLE_FIELD: Record<string, 'password' | 'code'> = { wrong_password: 'password', rate_limited: 'password', invalid_code: 'code' }

function Disable({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [password, setPassword] = useState('')
  const [code, setCode] = useState('')
  const disable = useMutation({
    mutationFn: () => api.disableTotp(password, code),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['me'] })
      toast.success(t('totp.disabled'))
      onDone()
    },
  })
  const field = disable.error instanceof ApiError ? DISABLE_FIELD[disable.error.code] : undefined
  const message = disable.error ? accountError(disable.error, t) : null
  const submit = (e: FormEvent) => {
    e.preventDefault()
    disable.mutate()
  }
  return (
    <DialogContent closeLabel={t('common.close')}>
      <DialogHeader>
        <DialogTitle>{t('totp.disableTitle')}</DialogTitle>
        <DialogDescription>{t('totp.disableHelp')}</DialogDescription>
      </DialogHeader>
      <form onSubmit={submit} className="space-y-4" noValidate>
        {message && !field && (
          <p role="alert" className="text-sm text-destructive">
            {message}
          </p>
        )}
        <Field id={`${id}-password`} label={t('auth.current')} error={field === 'password' ? message : null}>
          <PasswordInput
            id={`${id}-password`}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            aria-invalid={field === 'password'}
            aria-describedby={describedBy(`${id}-password`, false, field === 'password')}
          />
        </Field>
        <Field id={`${id}-code`} label={t('auth.code')} help={t('auth.codeHelp')} error={field === 'code' ? message : null}>
          <CodeInput id={`${id}-code`} value={code} onChange={setCode} invalid={field === 'code'} />
        </Field>
        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button type="button" variant="secondary" onClick={onDone}>
            {t('admin.cancel')}
          </Button>
          <Button type="submit" disabled={disable.isPending}>
            {disable.isPending && <Loader2 aria-hidden className="animate-spin" />}
            {t('totp.disable')}
          </Button>
        </div>
      </form>
    </DialogContent>
  )
}
