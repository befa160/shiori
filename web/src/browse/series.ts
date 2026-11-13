import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/api/client'
import type { SeriesPatch } from '@/api/types'

// Listes qui montrent le titre, la couverture, la progression ou le statut d'une série :
// redemandées après un changement.
export const LISTS = [['series'], ['libraries'], ['continue']]

/** Modifie une série : le détail renvoyé remplace le cache, les listes sont redemandées. */
export function usePatchSeries(id: number) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (patch: SeriesPatch) => api.patchSeries(id, patch),
    onSuccess: (detail) => {
      queryClient.setQueryData(['series', id], detail)
      for (const queryKey of LISTS) void queryClient.invalidateQueries({ queryKey, exact: queryKey[0] === 'series' })
    },
  })
}

/** Adresses `http(s)` seulement : une valeur venue d'une archive n'est jamais un lien d'un autre type. */
export function webLinks(links: string[]): URL[] {
  return links.flatMap((link) => {
    try {
      const url = new URL(link)
      return url.protocol === 'https:' || url.protocol === 'http:' ? [url] : []
    } catch {
      return []
    }
  })
}
