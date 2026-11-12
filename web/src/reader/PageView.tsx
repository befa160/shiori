import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ApiError, request } from '@/api/client'
import type { PageInfo } from '@/api/types'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'
import { fittedWidth, pageUrl } from './controls'
import type { Half } from './views'

/** La cause d'un échec d'affichage, demandée au serveur (une balise <img> ne la donne pas). */
async function pageError(pageId: string): Promise<string> {
  try {
    await request(`/pages/${encodeURIComponent(pageId)}/image`)
    return i18n.t('reader.pageFailed')
  } catch (error) {
    return error instanceof ApiError ? errorText(error, i18n.t) : i18n.t('reader.pageFailed')
  }
}

/** Une page à sa place (principe 4) : l'image, ou une erreur explicite de la même taille,
 * jamais une autre page. Un rendu redimensionné qui échoue est retenté en originale. `half` :
 * seulement une moitié d'une double page, coupée côté client dans la même image. */
export function PageView({
  page,
  total,
  view,
  half,
}: {
  page: PageInfo
  total: number
  view: { width: number; height: number }
  half?: Half
}) {
  const { t } = useTranslation()
  const [original, setOriginal] = useState(false)
  const [error, setError] = useState<string | null>(page.status === 'broken' ? t('errors.page_broken') : null)
  // Une moitié se dimensionne comme une page deux fois moins large ; l'image entière fait le double.
  const shape = half && page.width ? { ...page, width: page.width / 2 } : page
  const width = fittedWidth(view.width, view.height, shape)
  const box = {
    width,
    aspectRatio: shape.width && shape.height ? `${shape.width} / ${shape.height}` : undefined,
  }
  const page_ = t('reader.pageOf', { current: page.ordinal + 1, total })
  const label = half ? `${page_} (${t(`reader.halves.${half}`)})` : page_
  if (error) {
    return (
      <div
        role="img"
        aria-label={`${label} : ${error}`}
        style={{ ...box, aspectRatio: box.aspectRatio ?? '2 / 3' }}
        className="flex max-h-dvh max-w-full items-center justify-center rounded-md border border-destructive/40 bg-card p-6 text-center"
      >
        <p className="text-destructive">{error}</p>
      </div>
    )
  }
  const url = pageUrl(page, half ? { width: view.width * 2, height: view.height } : view, original)
  const onError = async () => {
    if (!original && url.includes('?')) {
      setOriginal(true)
      return
    }
    setError(await pageError(page.id))
  }
  if (half) {
    return (
      <div style={box} className="max-h-dvh max-w-full overflow-hidden">
        <img
          src={url}
          alt={label}
          draggable={false}
          // L'image entière, deux fois plus large que la boîte, décalée pour n'en montrer qu'une moitié.
          style={{ width: '200%', maxWidth: 'none', height: '100%', marginLeft: half === 'right' ? '-100%' : 0 }}
          className="select-none"
          onError={onError}
        />
      </div>
    )
  }
  return (
    <img
      src={url}
      alt={label}
      style={box}
      draggable={false}
      className="max-h-dvh max-w-full object-contain select-none"
      onError={onError}
    />
  )
}
