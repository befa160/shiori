import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { type Scope, usePreference } from '@/account/preferences'
import type { Theme } from '@/api/types'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { errorText } from '@/i18n/errors'

const THEMES: Theme[] = ['system', 'dark', 'light', 'oled', 'sepia']

/** Aperçu dessiné avec les jetons du thème lui-même (`data-theme`) : fond, carte, texte, ambre. */
function Swatch({ theme }: { theme: Exclude<Theme, 'system'> }) {
  return (
    <span data-theme={theme} className="flex flex-1 flex-col justify-end gap-1 bg-background p-1.5">
      <span className="flex items-center gap-1 rounded-sm bg-card p-1">
        <span className="h-1.5 flex-1 rounded-full bg-foreground/80" />
        <span className="size-2 rounded-full bg-primary" />
      </span>
    </span>
  )
}

/** Thème de l'interface (du compte, ou défaut de l'instance) ; le lecteur reste toujours sombre. */
export function ThemeSetting({ scope = 'account' }: { scope?: Scope }) {
  const { t } = useTranslation()
  const id = useId()
  const { value: saved, save } = usePreference('theme', scope)
  const choose = (theme: Theme) =>
    save.mutate(theme, { onError: (error) => toast.error(t('admin.themeNotSaved'), { description: errorText(error, t) }) })
  const value = save.isPending ? save.variables : saved
  if (value === undefined) return null
  return (
    <div className="space-y-3">
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {t(scope === 'instance' ? 'admin.themeHelp' : 'account.themeHelp')}
      </p>
      <ToggleGroup
        type="single"
        spacing={2}
        value={value}
        onValueChange={(v) => v && choose(v as Theme)}
        aria-label={t('admin.theme')}
        aria-describedby={`${id}-help`}
        className="grid w-full grid-cols-3 sm:grid-cols-5"
      >
        {THEMES.map((theme) => (
          <ToggleGroupItem key={theme} value={theme} className="h-auto flex-col items-stretch gap-1.5 p-1.5">
            {/* Décor : le nom du thème suffit aux lecteurs d'écran. */}
            <span aria-hidden className="flex h-12 overflow-hidden rounded-sm ring-1 ring-border">
              {theme === 'system' ? (
                <>
                  <Swatch theme="dark" />
                  <Swatch theme="light" />
                </>
              ) : (
                <Swatch theme={theme} />
              )}
            </span>
            {t(`admin.themes.${theme}`)}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </div>
  )
}
