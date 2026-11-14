import { useQuery } from '@tanstack/react-query'
import { useEffect, useSyncExternalStore } from 'react'
import { api } from '@/api/client'
import type { Theme } from '@/api/types'

/** Thème réellement affiché : `system` devient sombre ou clair selon l'appareil. */
export type ShownTheme = Exclude<Theme, 'system'>

const STORAGE_KEY = 'shiori.theme'
const LIGHT = '(prefers-color-scheme: light)'

export function resolveTheme(theme: Theme, prefersLight: boolean): ShownTheme {
  if (theme !== 'system') return theme
  return prefersLight ? 'light' : 'dark'
}

const shownListeners = new Set<() => void>()

/** Thème affiché (`dark` tant que rien n'est appliqué), pour ce qui ne lit pas les jetons CSS
 * (les notifications de sonner ont leurs propres couleurs de texte). */
export function useShownTheme(): ShownTheme {
  return useSyncExternalStore(
    (onChange) => {
      shownListeners.add(onChange)
      return () => shownListeners.delete(onChange)
    },
    () => (document.documentElement.dataset.theme as ShownTheme | undefined) ?? 'dark',
  )
}

/** Pose le thème sur <html> (jetons de `index.css`, variantes `dark:` de shadcn) et la couleur
 * de la barre du navigateur ; le garde pour le prochain chargement. */
export function applyTheme(theme: ShownTheme) {
  const html = document.documentElement
  html.dataset.theme = theme
  html.classList.toggle('dark', theme === 'dark' || theme === 'oled')
  const background = getComputedStyle(html).getPropertyValue('--background').trim()
  if (background) document.querySelector('meta[name="theme-color"]')?.setAttribute('content', background)
  for (const listener of shownListeners) listener()
  try {
    localStorage.setItem(STORAGE_KEY, theme)
  } catch {
    // Navigation privée ou stockage refusé : seul le prochain chargement clignotera.
  }
}

/** Au démarrage, avant la réponse du serveur : le dernier thème affiché, pour ne pas clignoter. */
export function applyStoredTheme() {
  let stored: string | null = null
  try {
    stored = localStorage.getItem(STORAGE_KEY)
  } catch {
    // idem
  }
  if (stored === 'dark' || stored === 'light' || stored === 'oled' || stored === 'sepia') applyTheme(stored)
}

function subscribe(onChange: () => void) {
  const query = matchMedia(LIGHT)
  query.addEventListener('change', onChange)
  return () => query.removeEventListener('change', onChange)
}

/** Applique le thème enregistré sur le serveur (suit l'appareil en direct pour `system`) ;
 * `forced` l'impose, pour le lecteur qui reste toujours sombre. */
export function useTheme(forced?: ShownTheme) {
  const settings = useQuery({ queryKey: ['settings'], queryFn: ({ signal }) => api.settings({ signal }) })
  const prefersLight = useSyncExternalStore(subscribe, () => matchMedia(LIGHT).matches)
  const saved = settings.data?.theme
  useEffect(() => {
    if (forced) applyTheme(forced)
    else if (saved) applyTheme(resolveTheme(saved, prefersLight))
  }, [forced, saved, prefersLight])
}
