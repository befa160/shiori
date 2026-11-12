import { type QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type DataRouter, RouterProvider } from 'react-router'
import { Toaster } from '@/components/ui/sonner'

export function App({ router, queryClient }: { router: DataRouter; queryClient: QueryClient }) {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      {/* Hors des routes : le lecteur plein écran pourra aussi notifier. */}
      <Toaster />
    </QueryClientProvider>
  )
}
