import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { PageTurn } from '@/api/types'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { errorText } from '@/i18n/errors'

const TURNS: PageTurn[] = ['reading', 'ltr', 'rtl']

/** Sens par défaut pour tourner les pages (global, comme la langue) ; une série peut avoir le
 * sien, réglé dans le panneau « Affichage » du lecteur. */
export function PageTurnSetting() {
  const { t } = useTranslation()
  const id = useId()
  const queryClient = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: ({ signal }) => api.settings({ signal }) })
  const save = useMutation({
    mutationFn: (page_turn: PageTurn) => api.saveSettings({ page_turn }),
    onSuccess: (saved) => {
      queryClient.setQueryData(['settings'], saved)
      // Les réglages effectifs des séries et des tomes en dépendent.
      for (const queryKey of [['series'], ['volumes']]) void queryClient.invalidateQueries({ queryKey })
    },
    onError: (error) => toast.error(t('admin.pageTurnNotSaved'), { description: errorText(error, t) }),
  })
  const value = save.isPending ? save.variables : settings.data?.page_turn
  if (value === undefined) return null
  return (
    <div className="space-y-1">
      <p id={`${id}-label`} className="text-sm font-medium">
        {t('admin.pageTurn')}
      </p>
      <p id={`${id}-help`} className="text-sm text-muted-foreground">
        {t('admin.pageTurnHelp')}
      </p>
      <Select value={value} onValueChange={(v) => save.mutate(v as PageTurn)}>
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
