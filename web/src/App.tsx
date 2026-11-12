import { type QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type DataRouter, RouterProvider } from 'react-router'

export function App({ router, queryClient }: { router: DataRouter; queryClient: QueryClient }) {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}
