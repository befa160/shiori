import { useQueryClient } from '@tanstack/react-query'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import { type Scope, usePreference } from '@/account/preferences'
import type { PageTurn } from '@/api/types'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { toastFailure } from '@/i18n/errors'

const TURNS: PageTurn[] = ['reading', 'ltr', 'rtl']

/** Sens par défaut pour tourner les pages (du compte, ou défaut de l'instance) ; une série peut
 * avoir le sien, réglé dans le panneau « Affichage » du lecteur. */
export function PageTurnSetting({ scope = 'account' }: { scope?: Scope }) {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const { value: saved, save } = usePreference('page_turn', scope)
  const choose = (turn: PageTurn) =>
    save.mutate(turn, {
      // Les réglages effectifs des séries et des tomes en dépendent.
      onSuccess: () => {
        for (const queryKey of [['series'], ['volumes']]) void queryClient.invalidateQueries({ queryKey })
      },
      onError: toastFailure(t('admin.pageTurnNotSaved'), t),
    })
  const value = save.isPending ? save.variables : saved
  if (value === undefined) return null
  return (
    <div className="space-y-1">
      <p id={`${id}-label`} className="text-sm font-medium">
        {t('admin.pageTurn')}
      </p>
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {t(scope === 'instance' ? 'admin.pageTurnHelp' : 'account.pageTurnHelp')}
      </p>
      <Select value={value} onValueChange={(v) => choose(v as PageTurn)}>
        <SelectTrigger aria-labelledby={`${id}-label`} aria-describedby={`${id}-help`} className="w-full max-w-sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent position="popper" align="start">
          {TURNS.map((turn) => (
            <SelectItem key={turn} value={turn}>
              {t(`reader.pageTurns.${turn}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}
