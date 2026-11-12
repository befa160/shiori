import type { RouteObject } from 'react-router'
import { LibrariesPage } from '@/browse/LibrariesPage'
import { LibraryPage } from '@/browse/LibraryPage'
import { SeriesPage } from '@/browse/SeriesPage'
import { WithId } from '@/browse/parts'
import { Layout } from '@/pages/Layout'
import { NotFound } from '@/pages/NotFound'

export const routes: RouteObject[] = [
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
