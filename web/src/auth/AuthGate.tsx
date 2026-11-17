import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Loader2 } from 'lucide-react'
import { type FormEvent, type ReactNode, useEffect, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Outlet } from 'react-router'
import { ApiError, api } from '@/api/client'
import { PasswordForm } from '@/account/PasswordForm'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { setProgressUser } from '@/offline/progress'
import { describedBy } from './describedBy'
import { Field } from './Field'
import { PasswordInput } from './PasswordInput'
import { accountError, signedIn, useAuth } from './session'

/** Garde de toute l'application : installation tant qu'aucun compte n'existe, connexion sans
 * session, mot de passe provisoire à changer ; sinon l'écran demandé, à la même adresse (on y
 * revient une fois connecté). Sans réseau, l'application s'ouvre quand même : tomes téléchargés. */
export function AuthGate() {
  const { t } = useTranslation()
  const auth = useAuth()
  const userId = auth.data?.user?.id
  useEffect(() => {
    if (userId !== undefined) setProgressUser(userId)
  }, [userId])
  if (auth.isPending) return <div className="min-h-dvh bg-background" />
  if (auth.isError) {
    if (auth.error instanceof ApiError && auth.error.status === 0) return <Outlet />
    return (
      <AuthScreen title={t('loadFailed.title')}>
        <p role="alert" className="text-sm text-muted-foreground">
          {accountError(auth.error, t)}
        </p>
        <Button onClick={() => void auth.refetch()}>{t('common.retry')}</Button>
      </AuthScreen>
    )
  }
  const { setup_required, user } = auth.data
  if (setup_required) return <SetupPage />
  if (!user) return <LoginPage />
  if (user.must_change_password) return <ProvisionalPassword />
  return <Outlet />
}

/** Cadre des écrans hors session : logo, titre, carte centrée, zones sûres. */
function AuthScreen({ title, intro, children }: { title: string; intro?: string; children: ReactNode }) {
  return (
    <main className="flex min-h-dvh items-center justify-center bg-background pt-[max(1.5rem,env(safe-area-inset-top))] pr-[max(1rem,env(safe-area-inset-right))] pb-[max(1.5rem,env(safe-area-inset-bottom))] pl-[max(1rem,env(safe-area-inset-left))] text-foreground">
      <title>{`${title} · Shiori`}</title>
      <div className="w-full max-w-sm space-y-6">
        <div className="flex items-center justify-center gap-2 text-xl font-semibold">
          {/* Décoratif : le nom suit. */}
          <img src="/shiori-mark.svg" alt="" width={32} height={32} className="hidden dark:block" />
          <img src="/shiori-mark-light.svg" alt="" width={32} height={32} className="dark:hidden" />
          Shiori
        </div>
        <section className="space-y-4 rounded-md bg-card p-6">
          <h1 className="text-lg font-semibold">{title}</h1>
          {intro && <p className="text-sm text-muted-foreground">{intro}</p>}
          {children}
        </section>
      </div>
    </main>
  )
}

function Submit({ pending, children }: { pending: boolean; children: ReactNode }) {
  return (
    <Button type="submit" size="lg" disabled={pending} className="w-full">
      {pending && <Loader2 aria-hidden className="animate-spin" />}
      {children}
    </Button>
  )
}

function LoginPage() {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const login = useMutation({
    mutationFn: () => api.login({ username: username.trim(), password }),
    onSuccess: ({ user }) => signedIn(queryClient, user),
  })
  const submit = (e: FormEvent) => {
    e.preventDefault()
    login.mutate()
  }
  const error = login.error ? accountError(login.error, t) : null
  return (
    <AuthScreen title={t('auth.loginTitle')}>
      <form onSubmit={submit} className="space-y-4" noValidate>
        <Field id={`${id}-name`} label={t('auth.username')}>
          <Input
            id={`${id}-name`}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="username"
            autoCapitalize="none"
            spellCheck={false}
            required
            aria-invalid={Boolean(error)}
          />
        </Field>
        <Field id={`${id}-password`} label={t('auth.password')} error={error}>
          <PasswordInput
            id={`${id}-password`}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
            aria-invalid={Boolean(error)}
            aria-describedby={describedBy(`${id}-password`, false, Boolean(error))}
          />
        </Field>
        <Submit pending={login.isPending}>{t('auth.login')}</Submit>
      </form>
      <p className="text-sm text-muted-foreground">{t('auth.forgot')}</p>
    </AuthScreen>
  )
}

const SETUP_FIELD: Record<string, 'token' | 'name' | 'password'> = {
  invalid_setup_token: 'token',
  invalid_username: 'name',
  weak_password: 'password',
}

/** Premier compte (administrateur), protégé par le jeton affiché dans la console de Shiori. */
function SetupPage() {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const [token, setToken] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const setup = useMutation({
    mutationFn: () => api.setup({ token: token.trim(), username: username.trim(), password }),
    onSuccess: ({ user }) => signedIn(queryClient, user),
    // Quelqu'un d'autre vient de créer le compte : page de connexion.
    onError: (error) => error instanceof ApiError && error.code === 'already_set_up' && void queryClient.invalidateQueries({ queryKey: ['auth'] }),
  })
  const field = setup.error instanceof ApiError ? SETUP_FIELD[setup.error.code] : undefined
  const message = setup.error ? accountError(setup.error, t) : null
  const errorOf = (which: 'token' | 'name' | 'password') => (field === which ? message : null)
  const submit = (e: FormEvent) => {
    e.preventDefault()
    setup.mutate()
  }
  return (
    <AuthScreen title={t('auth.setupTitle')} intro={t('auth.setupIntro')}>
      <form onSubmit={submit} className="space-y-4" noValidate>
        {message && !field && (
          <p role="alert" className="text-sm text-destructive">
            {message}
          </p>
        )}
        <Field id={`${id}-token`} label={t('auth.token')} help={t('auth.tokenHelp')} error={errorOf('token')}>
          <Input
            id={`${id}-token`}
            value={token}
            onChange={(e) => setToken(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            className="font-mono"
            required
            aria-invalid={field === 'token'}
            aria-describedby={describedBy(`${id}-token`, true, field === 'token')}
          />
        </Field>
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
        <Submit pending={setup.isPending}>{t('auth.setup')}</Submit>
      </form>
    </AuthScreen>
  )
}

/** Mot de passe provisoire (compte créé par l'administrateur) : le changer avant tout le reste. */
function ProvisionalPassword() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  return (
    <AuthScreen title={t('auth.provisionalTitle')} intro={t('auth.provisionalIntro')}>
      <PasswordForm provisional onDone={() => void queryClient.invalidateQueries({ queryKey: ['auth'] })} />
    </AuthScreen>
  )
}
