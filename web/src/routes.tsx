import type { RouteObject } from 'react-router'
import { LibrariesPage } from '@/browse/LibrariesPage'
import { LibraryPage } from '@/browse/LibraryPage'
import { SeriesPage } from '@/browse/SeriesPage'
import { WithId } from '@/browse/parts'
import { Layout } from '@/pages/Layout'
import { LoadFailed, NotFound } from '@/pages/NotFound'

export const routes: RouteObject[] = [
  // Plein écran, hors du cadre commun.
  // Chargé à l'ouverture d'un tome seulement (son code, le virtualiseur, le curseur…) :
  // l'écran d'accueil s'affiche sans lui.
  {
    path: '/read/:volumeId',
    lazy: async () => ({ Component: (await import('@/reader/Reader')).ReaderRoute }),
    // Le temps de charger le lecteur : l'écran noir du lecteur, rien d'autre.
    HydrateFallback: () => <div className="fixed inset-0 bg-black" />,
    ErrorBoundary: LoadFailed,
  },
  {
    path: '/',
    element: <Layout />,
    children: [
      { index: true, element: <LibrariesPage /> },
      { path: 'libraries/:id', element: <WithId>{(id) => <LibraryPage id={id} />}</WithId> },
      { path: 'series/:id', element: <WithId>{(id) => <SeriesPage id={id} />}</WithId> },
      { path: '*', element: <NotFound /> },
    ],
  },
]
