import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter } from 'react-router'
import { App } from '@/App'
import '@/i18n'
import { createQueryClient } from '@/lib/query'
import { applyStoredTheme } from '@/lib/theme'
import { routes } from '@/routes'
import './index.css'

applyStoredTheme()

// Service worker (lot 6.5a) : en production seulement, il garderait sinon d'anciens fichiers du
// serveur de développement.
if (import.meta.env.PROD && 'serviceWorker' in navigator) void navigator.serviceWorker.register('/sw.js')

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App router={createBrowserRouter(routes)} queryClient={createQueryClient()} />
  </StrictMode>,
)
