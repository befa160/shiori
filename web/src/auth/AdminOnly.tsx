import { Outlet } from 'react-router'
import { NotFound } from '@/pages/NotFound'
import { useIsAdmin } from './session'

/** Écrans d'administration : un lecteur voit « introuvable », comme le serveur lui répondrait. */
export function AdminOnly() {
  return useIsAdmin() ? <Outlet /> : <NotFound />
}
