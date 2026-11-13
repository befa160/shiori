import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import type { Library } from '@/api/types'
import { errorText } from '@/i18n/errors'

/** Lance le scan d'une bibliothèque ; le résultat se dit par une notification, et le suivi de
 * la passe en cours repart (il n'interroge le serveur que pendant un scan). */
export function useStartScan() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ library, options }: { library: Pick<Library, 'id' | 'name'>; options?: { deep?: boolean; rehash?: boolean } }) =>
      api.startScan(library.id, options),
    onSuccess: (_, { library }) => {
      toast.success(t('admin.scanStarted', { name: library.name }))
      void queryClient.invalidateQueries({ queryKey: ['scans', 'current'] })
    },
    onError: (error) => toast.error(t('admin.scanNotStarted'), { description: errorText(error, t) }),
  })
}
