import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { toast } from 'sonner'
import { api } from '@/api/client'
import i18n, { type Language, browserLanguage } from '.'
import { errorText } from './errors'

/** Langue enregistrée sur le serveur (global tant qu'il n'y a pas de comptes), appliquée à
 * l'interface ; sans choix enregistré, la langue du navigateur. */
export function useLanguage() {
  const queryClient = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: ({ signal }) => api.settings({ signal }) })
  const saved = settings.data?.language
  useEffect(() => {
    if (settings.isSuccess) void i18n.changeLanguage(saved ?? browserLanguage())
  }, [settings.isSuccess, saved])

  const save = useMutation({
    mutationFn: (language: Language) => api.saveSettings({ language }),
    // La langue change tout de suite ; un échec d'enregistrement la remet comme avant.
    onMutate: (language) => {
      const previous = i18n.language
      void i18n.changeLanguage(language)
      return previous
    },
    onError: async (error, _language, previous) => {
      await i18n.changeLanguage(previous)
      // `i18n.t` et non le `t` du rendu : celui-ci parle encore la langue qui vient d'échouer.
      toast.error(i18n.t('layout.languageNotSaved'), { description: errorText(error, i18n.t) })
    },
    onSuccess: (saved) => queryClient.setQueryData(['settings'], saved),
  })
  return { choose: (language: Language) => save.mutate(language) }
}
