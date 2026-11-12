// Composants que les deux modes du lecteur (paginé, vertical) partagent : barre de commandes et
// curseur de pages. Leurs crochets (taille de l'écran, touches) sont dans `hooks.ts`.
import { ChevronLeft, Columns2 } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import type { ReadingMode } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from '@/lib/utils'

/** Barre de commandes : retour, titre et réglages en haut ; `children` en bas. */
export function Toolbar({
  chrome,
  title,
  back,
  display,
  end,
  children,
}: {
  chrome: boolean
  title: string
  back: string
  display?: ReactNode
  /** Action de fin de tome, sur sa propre ligne au-dessus de la barre du bas. */
  end?: ReactNode
  children: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <div
      role="toolbar"
      aria-label={t('reader.controls')}
      className={cn('pointer-events-none fixed inset-0 flex flex-col justify-between', !chrome && 'invisible')}
    >
      {/* Marges de 8 px sur les côtés, 4 px en haut et en bas (l'anneau de focus reste à l'écran),
          ou la zone sûre si elle est plus grande (encoche, coins arrondis). */}
      <header className="pointer-events-auto flex items-center gap-2 bg-black/80 pt-[max(0.25rem,env(safe-area-inset-top))] pb-1 pr-[max(0.5rem,env(safe-area-inset-right))] pl-[max(0.5rem,env(safe-area-inset-left))]">
        <Button asChild variant="ghost">
          <Link to={back}>
            <ChevronLeft aria-hidden />
            {t('reader.back')}
          </Link>
        </Button>
        <p className="min-w-0 flex-1 truncate font-medium">{title}</p>
        {display}
      </header>
      <div className="mt-auto">
        {end && <div className="pointer-events-auto flex justify-center p-3">{end}</div>}
        <footer className="pointer-events-auto flex items-center gap-3 bg-black/80 pr-[max(0.5rem,env(safe-area-inset-right))] pt-1 pb-[max(0.25rem,env(safe-area-inset-bottom))] pl-[max(0.5rem,env(safe-area-inset-left))]">
          {children}
        </footer>
      </div>
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
      <span className="shrink-0 text-sm text-muted-foreground tabular-nums">{counter(dragged)}</span>
    </>
  )
}

/** Panneau Affichage : sens de lecture (retenu pour la série), puis les réglages propres au mode. */
export function DisplayOptions({
  mode,
  onMode,
  children,
}: {
  mode: ReadingMode
  onMode: (mode: ReadingMode) => void
  children?: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="ghost">
          <Columns2 aria-hidden />
          {t('reader.display')}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-72 space-y-4">
        <Choice
          legend={t('reader.mode')}
          value={mode}
          options={(['rtl', 'ltr', 'vertical'] as const).map((m) => [m, t(`reader.modes.${m}`)])}
          onChange={onMode}
        />
        {children}
      </PopoverContent>
    </Popover>
  )
}

/** Choix exclusif sous une légende (boutons radio de Radix, état actif en ambre). */
export function Choice<T extends string>({
  legend,
  value,
  options,
  onChange,
}: {
  legend: string
  value: T
  options: [T, string][]
  onChange: (value: T) => void
}) {
  return (
    <fieldset className="space-y-2">
      <legend className="text-sm text-muted-foreground">{legend}</legend>
      <ToggleGroup type="single" value={value} onValueChange={(v) => v && onChange(v as T)} className="w-full">
        {options.map(([v, label]) => (
          <ToggleGroupItem key={v} value={v} className="flex-1">
            {label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </fieldset>
  )
}
