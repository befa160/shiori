import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/api/client'
import type { Me, Settings } from '@/api/types'

export const useSettings = () => useQuery({ queryKey: ['settings'], queryFn: ({ signal }) => api.settings({ signal }) })

export const useMe = () => useQuery({ queryKey: ['me'], queryFn: ({ signal }) => api.me({ signal }) })

/** `account` : le choix du compte connecté ; `instance` : le défaut des comptes sans choix (Administration). */
export type Scope = 'account' | 'instance'
type Key = 'language' | 'theme' | 'page_turn'

/** Réglage effectif (celui du compte, sinon le défaut de l'instance) et de quoi l'enregistrer.
 * `value` reste `undefined` tant qu'il n'est pas connu. Sans réseau, le compte est inconnu : le
 * défaut de l'instance (copie hors ligne) s'applique. */
export function usePreference<K extends Key>(key: K, scope: Scope = 'account') {
  const queryClient = useQueryClient()
  const settings = useSettings()
  const me = useMe()
  const save = useMutation({
    mutationFn: (value: Settings[K]): Promise<Settings | Me> => (scope === 'instance' ? api.saveSettings({ [key]: value }) : api.saveMe({ [key]: value })),
    onSuccess: (saved) => queryClient.setQueryData([scope === 'instance' ? 'settings' : 'me'], saved),
  })
  const ready = settings.isSuccess && (scope === 'instance' || !me.isPending)
  const value = scope === 'instance' ? settings.data?.[key] : (me.data?.preferences[key] ?? settings.data?.[key])
  return { value: ready ? (value as Settings[K]) : undefined, ready, save }
}
