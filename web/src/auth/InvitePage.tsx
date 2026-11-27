import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { type FormEvent, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useNavigate, useParams } from 'react-router'
import { ApiError, api } from '@/api/client'
import type { Role } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AuthScreen, Submit } from './AuthGate'
import { describedBy } from './describedBy'
import { Field } from './Field'
import { PasswordInput } from './PasswordInput'
import { accountError, signedIn } from './session'

/** Lien d'invitation (`/invite/<jeton>`, lot 1.4b) : la personne choisit son nom et son mot de
 * passe, le compte a le rôle et les bibliothèques prévus par l'administrateur. Hors de la garde :
 * il s'ouvre sans session. */
export function InvitePage() {
  const { token = '' } = useParams()
  const check = useQuery({
    queryKey: ['invite', token],
    queryFn: ({ signal }) => api.checkInvite(token, { signal }),
    staleTime: Infinity,
    retry: false,
  })
  if (check.isPending) return <div className="min-h-dvh bg-background" />
  if (check.isError) return <Invalid error={check.error} onRetry={() => void check.refetch()} />
  return <InviteForm token={token} role={check.data.role} />
}

/** Lien inconnu, expiré, utilisé ou révoqué ; ou serveur injoignable (réessayer). */
function Invalid({ error, onRetry }: { error: Error; onRetry?: () => void }) {
  const { t } = useTranslation()
  const spent = error instanceof ApiError && error.code === 'invite_invalid'
  return (
    <AuthScreen title={t(spent ? 'invite.invalidTitle' : 'loadFailed.title')}>
      <p role="alert" className="text-sm text-muted-foreground">
        {spent ? t('invite.invalidHelp') : accountError(error, t)}
      </p>
      {!spent && onRetry && <Button onClick={onRetry}>{t('common.retry')}</Button>}
      <Button asChild variant={spent ? 'default' : 'secondary'} className="w-full">
        <Link to="/">{t('invite.toLogin')}</Link>
      </Button>
    </AuthScreen>
  )
}

const FIELD: Record<string, 'name' | 'password'> = {
  invalid_username: 'name',
  name_taken: 'name',
  weak_password: 'password',
}

function InviteForm({ token, role }: { token: string; role: Role }) {
  const { t } = useTranslation()
  const id = useId()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const accept = useMutation({
    mutationFn: () => api.acceptInvite(token, { username: username.trim(), password }),
    onSuccess: ({ user }) => {
      signedIn(queryClient, user)
      void navigate('/', { replace: true })
    },
  })
  if (accept.error instanceof ApiError && accept.error.code === 'invite_invalid') return <Invalid error={accept.error} />
  const field = accept.error instanceof ApiError ? FIELD[accept.error.code] : undefined
  const message = accept.error ? accountError(accept.error, t) : null
  const errorOf = (which: 'name' | 'password') => (field === which ? message : null)
  const submit = (e: FormEvent) => {
    e.preventDefault()
    accept.mutate()
  }
  return (
    <AuthScreen title={t('invite.title')} intro={t(`invite.intro.${role}`)}>
      <form onSubmit={submit} className="space-y-4" noValidate>
        {message && !field && (
          <p role="alert" className="text-sm text-destructive">
            {message}
          </p>
        )}
        <Field id={`${id}-name`} label={t('auth.username')} help={t('auth.usernameHelp')} error={errorOf('name')}>
          <Input
            id={`${id}-name`}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoCapitalize="none"
            spellCheck={false}
            required
            aria-invalid={field === 'name'}
            aria-describedby={describedBy(`${id}-name`, true, field === 'name')}
          />
        </Field>
        <Field id={`${id}-password`} label={t('auth.password')} help={t('auth.passwordHelp')} error={errorOf('password')}>
          <PasswordInput
            id={`${id}-password`}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            required
            minLength={8}
            aria-invalid={field === 'password'}
            aria-describedby={describedBy(`${id}-password`, true, field === 'password')}
          />
        </Field>
        <Submit pending={accept.isPending}>{t('auth.setup')}</Submit>
      </form>
    </AuthScreen>
  )
}
