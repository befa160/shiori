// Crochets partagés par les deux modes du lecteur : taille de l'écran et touches Échap / Tab.
import { useEffect, useState } from 'react'

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
