import { useMutation } from '@tanstack/react-query'
import { Loader2 } from 'lucide-react'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ApiError, api } from '@/api/client'
import { describedBy } from '@/auth/describedBy'
import { Field } from '@/auth/Field'
import { PasswordInput } from '@/auth/PasswordInput'
import { accountError } from '@/auth/session'
import { Button } from '@/components/ui/button'

const FIELD_OF: Record<string, 'current' | 'next'> = { wrong_password: 'current', weak_password: 'next', rate_limited: 'current' }

/** Changer son mot de passe : l'actuel (ou le provisoire), le nouveau deux fois. Les autres
 * appareils du compte sont déconnectés par le serveur. */
export function PasswordForm({ onDone, provisional = false }: { onDone: () => void; provisional?: boolean }) {
  const { t } = useTranslation()
  const id = useId()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [confirm, setConfirm] = useState('')
  const [mismatch, setMismatch] = useState(false)
  const change = useMutation({
    mutationFn: () => api.changePassword(current, next),
    onSuccess: () => {
      setCurrent('')
      setNext('')
      setConfirm('')
      onDone()
    },
  })
  const field = change.error instanceof ApiError ? FIELD_OF[change.error.code] : undefined
  const message = change.error ? accountError(change.error, t) : null
  const errorOf = (which: 'current' | 'next') => (field === which ? message : null)
  const submit = (e: FormEvent) => {
    e.preventDefault()
    setMismatch(next !== confirm)
    if (next === confirm) change.mutate()
  }
  const confirmError = mismatch ? t('auth.mismatch') : null
  return (
    <form onSubmit={submit} className="space-y-4" noValidate>
      {message && !field && (
        <p role="alert" className="text-sm text-destructive">
          {message}
        </p>
      )}
      <Field id={`${id}-current`} label={t(provisional ? 'auth.provisional' : 'auth.current')} error={errorOf('current')}>
        <PasswordInput
          id={`${id}-current`}
          value={current}
          onChange={(e) => setCurrent(e.target.value)}
          autoComplete="current-password"
          required
          aria-invalid={field === 'current'}
          aria-describedby={describedBy(`${id}-current`, false, field === 'current')}
        />
      </Field>
      <Field id={`${id}-next`} label={t('auth.new')} help={t('auth.passwordHelp')} error={errorOf('next')}>
        <PasswordInput
          id={`${id}-next`}
          value={next}
          onChange={(e) => setNext(e.target.value)}
          autoComplete="new-password"
          required
          minLength={8}
          aria-invalid={field === 'next'}
          aria-describedby={describedBy(`${id}-next`, true, field === 'next')}
        />
      </Field>
      <Field id={`${id}-confirm`} label={t('auth.confirm')} error={confirmError}>
        <PasswordInput
          id={`${id}-confirm`}
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          autoComplete="new-password"
          required
          aria-invalid={mismatch}
          aria-describedby={describedBy(`${id}-confirm`, false, mismatch)}
        />
      </Field>
      <Button type="submit" disabled={change.isPending}>
        {change.isPending && <Loader2 aria-hidden className="animate-spin" />}
        {t('auth.changePassword')}
      </Button>
    </form>
  )
}
