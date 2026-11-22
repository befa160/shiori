import type { TFunction } from 'i18next'
import type { SeriesVolume } from '@/api/types'

/** Ce qui nomme un tome (gardé aussi avec un tome téléchargé, pour le nommer hors ligne). */
export type VolumeName = Pick<SeriesVolume, 'kind' | 'number' | 'number_end' | 'file_name'>

/** Nom d'un tome dans sa série, d'après le parseur de noms du serveur, dans la langue donnée. */
export function volumeLabel(v: VolumeName, t: TFunction, language: string): string {
  if (v.number === null) {
    return v.kind === 'special' ? t('volume.special') : v.file_name
  }
  const n = new Intl.NumberFormat(language)
  const range = v.number_end === null ? n.format(v.number) : `${n.format(v.number)}–${n.format(v.number_end)}`
  const count = v.number_end === null ? 1 : 2
  switch (v.kind) {
    case 'volume':
      return t('volume.volume', { range, count })
    case 'chapter':
      return t('volume.chapter', { range, count })
    case 'special':
      return t('volume.specialNumber', { range })
    case 'unknown':
      return t('volume.unknown', { range })
  }
}

/** Titre d'un tome à montrer après son numéro, ou `null` s'il n'apporte rien : les ComicInfo
 * répètent souvent la série et le numéro (« Shangri-la Frontier - Tome 12 »). Le nom de la série
 * en tête est retiré ; ce qui n'est plus qu'un numéro de tome ou de chapitre est masqué. */
export function volumeTitle(title: string | undefined, series: string): string | null {
  if (!title) return null
  let rest = title.trim()
  if (rest.toLocaleLowerCase().startsWith(series.toLocaleLowerCase())) rest = rest.slice(series.length)
  rest = rest.replace(/^[\s\-–—:·,.]+/, '').trim()
  const onlyNumber = /^(tome|t\.?|vol(ume)?\.?|chap(itre|ter)?\.?|ch\.?|#)?\s*\d+([.,]\d+)?$/i
  return rest === '' || onlyNumber.test(rest) ? null : rest
}
