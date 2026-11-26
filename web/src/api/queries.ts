import { useQuery } from '@tanstack/react-query'
import { api } from './client'

// Clés de cache partagées : la même donnée n'est demandée qu'une fois entre deux écrans.
export const useLibraries = () =>
  useQuery({ queryKey: ['libraries'], queryFn: ({ signal }) => api.libraries({ signal }) })

export const useLibrarySeries = (id: number) =>
  useQuery({ queryKey: ['libraries', id, 'series'], queryFn: ({ signal }) => api.librarySeries(id, { signal }) })

export const useAllSeries = () =>
  useQuery({ queryKey: ['series'], queryFn: ({ signal }) => api.allSeries({ signal }) })

// Redemandée à chaque affichage de l'accueil et à chaque retour sur l'application (`staleTime: 0`) :
// une lecture ou une série marquée lue sur un autre appareil s'y voit tout de suite (retour du
// 2026-11-21 : un tome restait affiché jusqu'à 30 s, le délai par défaut).
export const useContinue = () =>
  useQuery({ queryKey: ['continue'], queryFn: ({ signal }) => api.continueReading({ signal }), staleTime: 0 })

export const useSeries = (id: number | null) =>
  useQuery({
    queryKey: ['series', id],
    queryFn: ({ signal }) => api.series(id as number, { signal }),
    enabled: id !== null,
  })

export const useVolume = (id: string) =>
  useQuery({ queryKey: ['volumes', id], queryFn: ({ signal }) => api.volume(id, { signal }) })

export const usePages = (volumeId: string) =>
  useQuery({ queryKey: ['volumes', volumeId, 'pages'], queryFn: ({ signal }) => api.pages(volumeId, { signal }) })

/** Passe de scan en cours (`null` : rien ne tourne), redemandée chaque seconde tant qu'elle tourne.
 * `idlePoll` : au repos aussi, pour voir un scan lancé ailleurs (autre appareil) là où il s'affiche. */
/** Marque-pages du compte : sous `['bookmarks']`, redemandés ensemble après chaque changement. */
export const useVolumeBookmarks = (volumeId: string) =>
  useQuery({ queryKey: ['bookmarks', 'volume', volumeId], queryFn: ({ signal }) => api.bookmarks(volumeId, { signal }) })

export const useSeriesBookmarks = (seriesId: number) =>
  useQuery({ queryKey: ['bookmarks', 'series', seriesId], queryFn: ({ signal }) => api.seriesBookmarks(seriesId, { signal }) })

export const useCurrentScan = (idlePoll: number | false = false) =>
  useQuery({
    queryKey: ['scans', 'current'],
    queryFn: ({ signal }) => api.currentScan({ signal }),
    refetchInterval: (query) => (query.state.data ? 1000 : idlePoll),
    staleTime: 0,
  })
