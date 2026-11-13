import type { ReadingStatus, SeriesSummary } from '@/api/types'

// Recherche, filtres et tris de la liste des séries, côté client (Q26) : la liste complète tient
// en une requête. Tout l'état vit dans l'adresse (`?q=&read=&status=&library=&sort=`).

export const READ_FILTERS = ['unread', 'in_progress', 'read'] as const satisfies readonly ReadingStatus[]
export const STATUS_FILTERS = ['ongoing', 'completed', 'hiatus', 'cancelled', 'unknown'] as const
export const SORTS = ['name', 'last_read', 'added'] as const

export type StatusFilter = (typeof STATUS_FILTERS)[number]
export type Sort = (typeof SORTS)[number]

export interface Filters {
  q: string
  read: ReadingStatus | null
  status: StatusFilter | null
  library: number | null
  sort: Sort
}

const pick = <T extends string>(allowed: readonly T[], value: string | null): T | null =>
  allowed.includes(value as T) ? (value as T) : null

/** Lit les filtres de l'adresse ; une valeur inconnue (lien abîmé) est ignorée. */
export function readFilters(params: URLSearchParams): Filters {
  const library = Number(params.get('library'))
  return {
    q: params.get('q') ?? '',
    read: pick(READ_FILTERS, params.get('read')),
    status: pick(STATUS_FILTERS, params.get('status')),
    library: Number.isInteger(library) && library > 0 ? library : null,
    sort: pick(SORTS, params.get('sort')) ?? 'name',
  }
}

/** Minuscules sans accents : « Été » trouve « ete », « ÉTÉ » et « été ». */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase()
}

const statusOf = (s: SeriesSummary): StatusFilter => s.publication_status ?? 'unknown'
// Dates ISO : l'ordre des chaînes est l'ordre chronologique.
const newestFirst = (a: string | null, b: string | null) => (b ?? '').localeCompare(a ?? '')

/** Séries qui passent les filtres, dans l'ordre demandé ; le serveur les envoie déjà par nom. */
export function applyFilters(series: SeriesSummary[], f: Filters): SeriesSummary[] {
  const words = fold(f.q).split(/\s+/).filter(Boolean)
  const kept = series.filter(
    (s) =>
      (f.read === null || s.reading_status === f.read) &&
      (f.status === null || statusOf(s) === f.status) &&
      (f.library === null || s.library_id === f.library) &&
      words.every((word) => fold(s.name).includes(word)),
  )
  // `sort` est stable : à égalité, l'ordre par nom du serveur reste.
  if (f.sort === 'last_read') return kept.sort((a, b) => newestFirst(a.last_read_at, b.last_read_at))
  if (f.sort === 'added') return kept.sort((a, b) => newestFirst(a.created_at, b.created_at))
  return kept
}

/** Un filtre au moins restreint la liste (le tri n'en est pas un). */
export const isFiltered = (f: Filters) => f.q.trim() !== '' || f.read !== null || f.status !== null || f.library !== null
