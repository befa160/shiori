import { ChevronsUpDown } from 'lucide-react'
import { useEffect, useRef, useState, type PointerEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { LETTERS } from './filters'

/** En dessous de ce nombre de séries, la liste tient en quelques écrans : pas d'index. */
export const INDEX_MIN = 30

// Entre la zone sûre du haut et la barre d'onglets, comme les marges de <main>.
const SPAN = 'top-[max(1.5rem,env(safe-area-inset-top))] bottom-[calc(var(--tabbar-height)+env(safe-area-inset-bottom)+1.5rem)]'

/** Index alphabétique d'une grille dont la première carte de chaque lettre porte `data-letter` :
 * colonne de lettres au bureau (souris, à partir de `lg`), poignée à faire glisser au toucher. */
export function AlphabetIndex({ letters }: { letters: Set<string> }) {
  return (
    <>
      <LetterBar letters={letters} />
      <ScrollHandle />
    </>
  )
}

const anchors = () => [...document.querySelectorAll<HTMLElement>('[data-letter]')]

/** Colonne `# A … Z` ; au toucher, réservée au clavier et aux lecteurs d'écran (visible au focus). */
function LetterBar({ letters }: { letters: Set<string> }) {
  const { t } = useTranslation()
  return (
    <nav aria-label={t('browse.index')} className={cn('fixed right-[max(0.25rem,env(safe-area-inset-right))] z-10 flex flex-col justify-center', SPAN)}>
      <ul className="sr-only focus-within:not-sr-only lg:pointer-fine:not-sr-only flex flex-col rounded-md bg-background/80 py-1 backdrop-blur-sm">
        {LETTERS.map((letter) => (
          <li key={letter}>
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={!letters.has(letter)}
              aria-label={t('browse.indexLetter', { letter })}
              onClick={() => anchors().find((a) => a.dataset.letter === letter)?.scrollIntoView({ block: 'start' })}
              className="text-muted-foreground hover:text-primary"
            >
              {letter}
            </Button>
          </li>
        ))}
      </ul>
    </nav>
  )
}

/** Lettre de la série en haut de l'écran (au premier tiers), ou la première de la grille. */
function letterOnScreen() {
  const all = anchors()
  return all.findLast((a) => a.getBoundingClientRect().top <= innerHeight / 3)?.dataset.letter ?? all[0]?.dataset.letter ?? '#'
}

/** Poignée de défilement : apparaît quand la page défile ; la faire glisser parcourt la liste et
 * montre la lettre atteinte. Doublon tactile du défilement natif, donc masquée aux lecteurs d'écran. */
function ScrollHandle() {
  const track = useRef<HTMLDivElement>(null)
  const [progress, setProgress] = useState(0)
  const [scrolling, setScrolling] = useState(false)
  // Lettre affichée pendant le glissement ; `null` hors glissement.
  const [letter, setLetter] = useState<string | null>(null)

  useEffect(() => {
    let timer = 0
    const onScroll = () => {
      const max = document.documentElement.scrollHeight - innerHeight
      setProgress(max > 0 ? Math.min(1, scrollY / max) : 0)
      setScrolling(true)
      clearTimeout(timer)
      timer = window.setTimeout(() => setScrolling(false), 1500)
    }
    addEventListener('scroll', onScroll, { passive: true })
    return () => {
      removeEventListener('scroll', onScroll)
      clearTimeout(timer)
    }
  }, [])

  const drag = (e: PointerEvent) => {
    const rect = track.current!.getBoundingClientRect()
    const fraction = Math.min(1, Math.max(0, (e.clientY - rect.top) / rect.height))
    scrollTo({ top: fraction * (document.documentElement.scrollHeight - innerHeight) })
    setLetter(letterOnScreen())
  }
  const shown = scrolling || letter !== null
  return (
    <div
      ref={track}
      aria-hidden
      data-testid="scroll-handle"
      className={cn('pointer-events-none fixed right-[env(safe-area-inset-right)] z-10 w-11 lg:pointer-fine:hidden', SPAN)}
    >
      <div
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId)
          drag(e)
        }}
        onPointerMove={(e) => e.currentTarget.hasPointerCapture(e.pointerId) && drag(e)}
        onPointerUp={() => setLetter(null)}
        onPointerCancel={() => setLetter(null)}
        style={{ top: `calc(${progress} * (100% - 3rem))` }}
        className={cn(
          'absolute right-0 flex h-12 w-11 touch-none items-center justify-center rounded-l-full border border-r-0 border-border bg-card text-muted-foreground shadow-md transition-opacity duration-200 motion-reduce:transition-none',
          shown ? 'pointer-events-auto opacity-100' : 'opacity-0',
        )}
      >
        <ChevronsUpDown className="size-5" />
        {letter !== null && (
          <span className="absolute right-full mr-3 flex size-14 items-center justify-center rounded-full bg-primary text-2xl font-semibold text-primary-foreground shadow-md">
            {letter}
          </span>
        )}
      </div>
    </div>
  )
}
