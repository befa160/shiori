import type { TFunction } from 'i18next'
import type { SeriesVolume } from '@/api/types'

/** Nom d'un tome dans sa série, d'après le parseur de noms du serveur, dans la langue donnée. */
export function volumeLabel(v: SeriesVolume, t: TFunction, language: string): string {
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
