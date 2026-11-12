import { useQuery } from '@tanstack/react-query'
import { api } from './client'

// Clés de cache partagées : la même donnée n'est demandée qu'une fois entre deux écrans.
export const useLibraries = () =>
  useQuery({ queryKey: ['libraries'], queryFn: ({ signal }) => api.libraries({ signal }) })

export const useLibrarySeries = (id: number) =>
  useQuery({ queryKey: ['libraries', id, 'series'], queryFn: ({ signal }) => api.librarySeries(id, { signal }) })

export const useAllSeries = () =>
  useQuery({ queryKey: ['series'], queryFn: ({ signal }) => api.allSeries({ signal }) })

export const useContinue = () =>
  useQuery({ queryKey: ['continue'], queryFn: ({ signal }) => api.continueReading({ signal }) })

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
