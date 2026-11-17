import { Eye, EyeOff } from 'lucide-react'
import { type ComponentProps, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Input } from '@/components/ui/input'

/** Mot de passe avec bouton « afficher » ; collage et gestionnaires de mots de passe permis. */
export function PasswordInput(props: Omit<ComponentProps<typeof Input>, 'type'>) {
  const { t } = useTranslation()
  const [shown, setShown] = useState(false)
  return (
    <div className="relative">
      <Input {...props} type={shown ? 'text' : 'password'} autoCapitalize="none" spellCheck={false} className="pr-12" />
      <button
        type="button"
        aria-label={t(shown ? 'auth.hide' : 'auth.show')}
        aria-pressed={shown}
        onClick={() => setShown((s) => !s)}
        className="absolute inset-y-0 right-0 flex w-11 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground"
      >
        {shown ? <EyeOff aria-hidden className="size-4" /> : <Eye aria-hidden className="size-4" />}
      </button>
    </div>
  )
}
