import type { LibraryKind, SeriesVolume } from '@/api/types'

const number = (n: number) => n.toLocaleString('fr-FR')

/** Nom d'un tome dans sa série, d'après le parseur de noms du serveur. */
export function volumeLabel(v: SeriesVolume): string {
  if (v.number === null) {
    return v.kind === 'special' ? 'Spécial' : v.file_name
  }
  const range = v.number_end === null ? number(v.number) : `${number(v.number)}–${number(v.number_end)}`
  const plural = v.number_end !== null
  switch (v.kind) {
    case 'volume':
      return `${plural ? 'Tomes' : 'Tome'} ${range}`
    case 'chapter':
      return `${plural ? 'Chapitres' : 'Chapitre'} ${range}`
    case 'special':
      return `Spécial ${range}`
    case 'unknown':
      return `N° ${range}`
  }
}

export const KIND_LABELS: Record<LibraryKind, string> = {
  manga: 'Manga',
  comics: 'BD / comics',
  webtoon: 'Webtoon',
}

/** « 1 série », « 12 séries ». */
export function count(n: number, one: string, many: string): string {
  return `${number(n)} ${n === 1 ? one : many}`
}
