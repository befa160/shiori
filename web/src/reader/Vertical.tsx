import { useWindowVirtualizer } from '@tanstack/react-virtual'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import type { PageInfo } from '@/api/types'
import { PageSlider, Toolbar } from './chrome'
import { MAX_COLUMN, stripHeight } from './controls'
import { useChromeKeys, useViewport } from './hooks'
import { PageView } from './PageView'

/** Mode webtoon : défilement vertical continu du document. Seules les bandes proches de l'écran
 * existent dans la page (12 760 bandes pour le webtoon réel) ; la place de chacune est réservée
 * d'après le scan, donc rien ne saute. Clavier : le défilement natif du navigateur. */
export function VerticalPages({ pages, title, back }: { pages: PageInfo[]; title: string; back: string }) {
  const { t, i18n } = useTranslation()
  const number = new Intl.NumberFormat(i18n.language)
  const view = useViewport()
  const [params, setParams] = useSearchParams()
  const [chrome, setChrome] = useState(false)
  useChromeKeys(chrome, setChrome)

  const column = Math.min(view.width, MAX_COLUMN)
  const heights = useMemo(() => pages.map((p) => stripHeight(p, column)), [pages, column])
  // Position de chaque bande : toutes les hauteurs sont connues d'avance (scan), aucun calcul
  // ne dépend de mesures du navigateur, dès le premier rendu.
  const tops = useMemo(() => {
    const out = [0]
    for (const h of heights) out.push(out[out.length - 1] + h)
    return out
  }, [heights])
  const jump = (index: number) => window.scrollTo({ top: tops[index] })
  // Règle pensée pour le React Compiler, que Shiori n'utilise pas : sans objet ici.
  // oxlint-disable-next-line react/incompatible-library
  const virtualizer = useWindowVirtualizer({
    count: pages.length,
    estimateSize: (i) => heights[i],
    overscan: 3,
  })
  // À l'ouverture (lien, rechargement) et quand la colonne change de largeur (rotation de
  // l'écran : toutes les hauteurs changent), on revient à la bande lue, connue par l'adresse.
  // Sans cela, la même position en pixels tombe sur une autre bande (mesuré : 408 → 199).
  const shownColumn = useRef<number | null>(null)
  useLayoutEffect(() => {
    if (shownColumn.current === column) return
    const first = shownColumn.current === null
    shownColumn.current = column
    virtualizer.measure()
    const index = pages.findIndex((p) => p.id === params.get('page'))
    if (index > 0 || !first) jump(Math.max(0, index))
  })

  // Bande lue = la première visible ; l'adresse la suit, sans réécrire l'historique à chaque pixel.
  const offset = virtualizer.scrollOffset ?? 0
  const current = virtualizer.getVirtualItems().find((item) => item.end > offset + 1)?.index ?? 0
  useEffect(() => {
    const id = pages[current]?.id
    if (!id || id === params.get('page')) return
    const timer = window.setTimeout(() => setParams({ page: id }, { replace: true }), 300)
    return () => window.clearTimeout(timer)
  }, [current, pages, params, setParams])

  return (
    <>
      <main
        className="relative mx-auto"
        style={{ width: column, height: virtualizer.getTotalSize() }}
        // Un clic (ou un tap qui n'a pas fait défiler) montre ou masque les commandes.
        onClick={() => setChrome((shown) => !shown)}
      >
        {virtualizer.getVirtualItems().map((item) => (
          <div
            key={item.key}
            className="absolute top-0 left-0 flex w-full justify-center"
            style={{ height: item.size, transform: `translateY(${item.start}px)` }}
          >
            <PageView page={pages[item.index]} total={pages.length} view={{ width: column, height: item.size }} strip />
          </div>
        ))}
      </main>
      {/* La bande retenue dans l'adresse (écrite à l'arrêt du défilement), pas la bande visible
          à chaque instant : sinon un lecteur d'écran annoncerait chaque bande traversée. */}
      <p className="sr-only" aria-live="polite">
        {t('reader.pageOf', { current: Math.max(0, pages.findIndex((p) => p.id === params.get('page'))) + 1, total: pages.length })}
      </p>
      <Toolbar chrome={chrome} title={title} back={back}>
        <PageSlider
          dir="ltr"
          total={pages.length}
          value={current}
          onCommit={jump}
          counter={(dragged) => `${number.format((dragged ?? current) + 1)} / ${number.format(pages.length)}`}
        />
      </Toolbar>
    </>
  )
}
