// Crochets partagés par les deux modes du lecteur : taille de l'écran, touches Échap / Tab,
// enregistrement de la progression.
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { api } from '@/api/client'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'

export function useViewport() {
  const read = () => ({ width: window.innerWidth, height: window.innerHeight })
  const [view, setView] = useState(read)
  useEffect(() => {
    const update = () => setView(read())
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])
  return view
}

/** Échap masque les commandes, Tab les montre (invisibles, elles sont hors du parcours clavier :
 * le clavier atteint tout sans piège). Les autres touches vont à `onKey`, sauf dans le panneau
 * Affichage ou dans la barre, où elles appartiennent au bouton ou au curseur qui a le focus. */
export function useChromeKeys(chrome: boolean, setChrome: (shown: boolean) => void, onKey?: (event: KeyboardEvent) => void) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement
      if (target.closest('[data-radix-popper-content-wrapper]')) return
      if (event.key === 'Escape') {
        // Le focus quitte la barre qui devient invisible : les touches retournent aux pages.
        if (target.closest('[role="toolbar"]')) target.blur()
        return setChrome(false)
      }
      if (event.key === 'Tab' && !chrome) {
        event.preventDefault()
        setChrome(true)
        requestAnimationFrame(() => document.querySelector<HTMLElement>('[role="toolbar"] a, [role="toolbar"] button')?.focus())
        return
      }
      if (target.closest('[role="toolbar"]')) return
      onKey?.(event)
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  })
}

/** Enregistre sur le serveur la page lue : 1 s après le dernier changement (feuilleter n'envoie
 * pas une requête par page), et tout de suite quand l'onglet passe en arrière-plan ou que le
 * lecteur se ferme (`keepalive` : la requête survit à la fermeture de l'onglet). */
export function useSaveProgress(volumeId: string, pageId: string | undefined) {
  const queryClient = useQueryClient()
  const state = useRef<{ pending?: string; saved?: string; flush?: () => void }>({})
  useEffect(() => {
    const s = state.current
    s.flush = () => {
      const page = s.pending
      s.pending = undefined
      if (!page || page === s.saved) return
      s.saved = page
      api
        .saveProgress(volumeId, page, { keepalive: true })
        // Les listes qui montrent la progression la redemanderont à leur prochain affichage.
        .then(() => {
          void queryClient.invalidateQueries({ queryKey: ['series'], refetchType: 'none' })
          void queryClient.invalidateQueries({ queryKey: ['volumes', volumeId], exact: true, refetchType: 'none' })
        })
        .catch((error: Error) => {
          if (s.saved === page) s.saved = undefined
          // Un seul message, même si chaque page suivante échoue aussi.
          toast.error(i18n.t('reader.progressNotSaved'), { id: 'progress', description: errorText(error, i18n.t) })
        })
    }
  }, [volumeId, queryClient])
  useEffect(() => {
    const s = state.current
    s.pending = pageId
    const timer = window.setTimeout(() => s.flush?.(), 1000)
    return () => window.clearTimeout(timer)
  }, [pageId])
  useEffect(() => {
    const s = state.current
    const onHide = () => {
      if (document.visibilityState === 'hidden') s.flush?.()
    }
    document.addEventListener('visibilitychange', onHide)
    return () => {
      document.removeEventListener('visibilitychange', onHide)
      s.flush?.()
    }
  }, [])
}
