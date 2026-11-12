import { useQuery } from '@tanstack/react-query'
import { api } from './client'

// Clés de cache partagées : la même donnée n'est demandée qu'une fois entre deux écrans.
export const useLibraries = () =>
  useQuery({ queryKey: ['libraries'], queryFn: ({ signal }) => api.libraries({ signal }) })

export const useLibrarySeries = (id: number) =>
  useQuery({ queryKey: ['libraries', id, 'series'], queryFn: ({ signal }) => api.librarySeries(id, { signal }) })

export const useSeries = (id: number) =>
  useQuery({ queryKey: ['series', id], queryFn: ({ signal }) => api.series(id, { signal }) })
