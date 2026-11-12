import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ApiError, request } from '@/api/client'
import type { PageInfo } from '@/api/types'
import i18n from '@/i18n'
import { errorText } from '@/i18n/errors'
import { fittedWidth, pageUrl } from './controls'

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
 * jamais une autre page. Un rendu redimensionné qui échoue est retenté en originale. */
export function PageView({
  page,
  total,
  view,
}: {
  page: PageInfo
  total: number
  view: { width: number; height: number }
}) {
  const { t } = useTranslation()
  const [original, setOriginal] = useState(false)
  const [error, setError] = useState<string | null>(page.status === 'broken' ? t('errors.page_broken') : null)
  const width = fittedWidth(view.width, view.height, page)
  const box = {
    width,
    aspectRatio: page.width && page.height ? `${page.width} / ${page.height}` : undefined,
  }
  const label = t('reader.pageOf', { current: page.ordinal + 1, total })
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
  const url = pageUrl(page, view, original)
  return (
    <img
      src={url}
      alt={label}
      style={box}
      draggable={false}
      className="max-h-dvh max-w-full object-contain select-none"
      onError={async () => {
        if (!original && url.includes('?')) {
          setOriginal(true)
          return
        }
        setError(await pageError(page.id))
      }}
    />
  )
}
