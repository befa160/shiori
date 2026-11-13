import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/api/client'
import type { SeriesDetail, SeriesPatch } from '@/api/types'

// Listes qui montrent le titre, la couverture, la progression ou le statut d'une série :
// redemandées après un changement.
export const LISTS = [['series'], ['libraries'], ['continue']]

/** Action sur une série qui renvoie son détail : il remplace le cache, les listes sont redemandées. */
export function useSeriesAction<T>(id: number, action: (input: T) => Promise<SeriesDetail>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: action,
    onSuccess: (detail) => {
      queryClient.setQueryData(['series', id], detail)
      for (const queryKey of LISTS) void queryClient.invalidateQueries({ queryKey, exact: queryKey[0] === 'series' })
    },
  })
}

/** Modifie une série (champs édités, verrouillés). */
export const usePatchSeries = (id: number) => useSeriesAction(id, (patch: SeriesPatch) => api.patchSeries(id, patch))

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
