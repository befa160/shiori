// Composants que les deux modes du lecteur (paginé, vertical) partagent : barre de commandes et
// curseur de pages. Leurs crochets (taille de l'écran, touches) sont dans `hooks.ts`.
import { ChevronLeft } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { cn } from '@/lib/utils'

/** Barre de commandes : retour, titre et réglages en haut ; `children` en bas. */
export function Toolbar({
  chrome,
  title,
  back,
  display,
  children,
}: {
  chrome: boolean
  title: string
  back: string
  display?: ReactNode
  children: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <div
      role="toolbar"
      aria-label={t('reader.controls')}
      className={cn('pointer-events-none fixed inset-0 flex flex-col justify-between', !chrome && 'invisible')}
    >
      <header className="pointer-events-auto flex items-center gap-2 bg-black/80 px-2 pt-safe pr-safe pl-safe">
        <Button asChild variant="ghost">
          <Link to={back}>
            <ChevronLeft aria-hidden />
            {t('reader.back')}
          </Link>
        </Button>
        <p className="min-w-0 flex-1 truncate font-medium">{title}</p>
        {display}
      </header>
      <footer className="pointer-events-auto flex items-center gap-3 bg-black/80 px-2 pr-safe pb-safe pl-safe">
        {children}
      </footer>
    </div>
  )
}

/** Curseur de pages : pendant qu'on le fait glisser, seul le numéro suit ; la page ne change
 * qu'au relâchement, sinon chaque page traversée serait demandée (et encodée) au serveur. */
export function PageSlider({
  dir,
  total,
  value,
  onCommit,
  counter,
}: {
  dir: 'rtl' | 'ltr'
  total: number
  value: number
  onCommit: (ordinal: number) => void
  /** Texte du compteur ; `dragged` : la page visée pendant le glissement. */
  counter: (dragged: number | null) => string
}) {
  const { t } = useTranslation()
  const [dragged, setDragged] = useState<number | null>(null)
  return (
    <>
      <Slider
        aria-label={t('reader.page')}
        dir={dir}
        min={0}
        max={total - 1}
        step={1}
        value={[dragged ?? value]}
        onValueChange={([v]) => setDragged(v)}
        onValueCommit={([v]) => {
          setDragged(null)
          onCommit(v)
        }}
      />
      <span className="shrink-0 pr-2 text-sm text-muted-foreground tabular-nums">{counter(dragged)}</span>
    </>
  )
}
