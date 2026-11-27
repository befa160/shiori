import type { ReadingStatus, SeriesSummary } from '@/api/types'
import { canonicalTag, tagSpellings } from '@/i18n/tags'

// Recherche, filtres et tris de la liste des séries, côté client (Q26) : la liste complète tient
// en une requête. Tout l'état vit dans l'adresse (`?q=&read=&status=&library=&tag=&tag=&sort=`).

export const READ_FILTERS = ['unread', 'in_progress', 'read'] as const satisfies readonly ReadingStatus[]
export const STATUS_FILTERS = ['ongoing', 'completed', 'hiatus', 'cancelled', 'unknown'] as const
/** Filtres de la liste : les statuts, plus « incomplète » (terminée, tomes manquants ; 1.4e). */
export const BROWSE_STATUS = [...STATUS_FILTERS, 'incomplete'] as const
export const SORTS = ['name', 'last_read', 'added'] as const

export type StatusFilter = (typeof BROWSE_STATUS)[number]
export type Sort = (typeof SORTS)[number]

export interface Filters {
  q: string
  read: ReadingStatus | null
  status: StatusFilter | null
  library: number | null
  /** Tags exigés, tous à la fois (`?tag=` répété). */
  tags: string[]
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
    status: pick(BROWSE_STATUS, params.get('status')),
    library: Number.isInteger(library) && library > 0 ? library : null,
    // Un tag par graphie repliée : `?tag=sf&tag=SF` (ou `Comedy` et `Comédie`) n'en fait qu'un.
    tags: [...new Map(params.getAll('tag').filter((t) => t.trim() !== '').map((t) => [tagKey(t), t])).values()],
    sort: pick(SORTS, params.get('sort')) ?? 'name',
  }
}

/** Minuscules sans accents : « Été » trouve « ete », « ÉTÉ » et « été ». */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase()
}

/** Ce que la recherche parcourt : titre affiché, autres titres, auteurs et tags (en anglais et en
 * français), sans accents. */
const searchable = (s: SeriesSummary) =>
  fold([s.name, ...s.alt_titles, ...s.authors, ...s.tags.flatMap(tagSpellings)].join('\n'))
/** Clé d'un tag : « Comedy » d'AniList, « Comédie » d'un ComicInfo et « comedie » sont le même. */
export const tagKey = (tag: string) => fold(canonicalTag(tag))
const hasTags = (s: SeriesSummary, wanted: string[]) => {
  const own = new Set(s.tags.map(tagKey))
  return wanted.every((t) => own.has(tagKey(t)))
}
/** Terminée mais des tomes manquent. */
export const incomplete = (s: Pick<SeriesSummary, 'publication_status' | 'missing_volumes'>) =>
  s.publication_status === 'completed' && (s.missing_volumes?.length ?? 0) > 0
const hasStatus = (s: SeriesSummary, f: StatusFilter) => (f === 'incomplete' ? incomplete(s) : (s.publication_status ?? 'unknown') === f)
// Dates ISO : l'ordre des chaînes est l'ordre chronologique.
const newestFirst = (a: string | null, b: string | null) => (b ?? '').localeCompare(a ?? '')

/** Lettres de l'index alphabétique ; `#` regroupe chiffres, symboles et autres écritures. */
export const LETTERS = ['#', ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'] as const
/** Initiale d'un titre pour l'index : « Été » → `E`, « 20th Century Boys » ou « 進撃 » → `#`. */
export function initial(name: string): string {
  // Le premier caractère seulement : appelé à chaque comparaison du tri.
  const first = fold(name.trimStart().charAt(0))
  return first >= 'a' && first <= 'z' ? first.toUpperCase() : '#'
}
const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true })
/** Ordre alphabétique de l'index : `#` d'abord, puis par lettre, sans tenir compte des accents ni
 * de la casse (le serveur trie octet par octet : « été » après « Zelda »), « Tome 2 » avant « Tome 10 ». */
export const byName = (a: { name: string }, b: { name: string }) =>
  rank(a.name) - rank(b.name) || collator.compare(a.name, b.name)
const rank = (name: string) => (LETTERS as readonly string[]).indexOf(initial(name))

/** Séries qui passent les filtres, dans l'ordre demandé. */
export function applyFilters(series: SeriesSummary[], f: Filters): SeriesSummary[] {
  const words = fold(f.q).split(/\s+/).filter(Boolean)
  const kept = series.filter(
    (s) =>
      (f.read === null || s.reading_status === f.read) &&
      (f.status === null || hasStatus(s, f.status)) &&
      (f.library === null || s.library_id === f.library) &&
      hasTags(s, f.tags) &&
      words.every((word) => searchable(s).includes(word)),
  ).sort(byName)
  // `sort` est stable : à égalité, l'ordre par nom reste.
  if (f.sort === 'last_read') return kept.sort((a, b) => newestFirst(a.last_read_at, b.last_read_at))
  if (f.sort === 'added') return kept.sort((a, b) => newestFirst(a.created_at, b.created_at))
  return kept
}

/** Première série de chaque lettre, à marquer dans la grille (`data-letter`) pour l'index. */
export function letterAnchors(sorted: { id: number; name: string }[]): Map<number, string> {
  const anchors = new Map<number, string>()
  let previous = ''
  for (const s of sorted) {
    const letter = initial(s.name)
    if (letter !== previous) anchors.set(s.id, letter)
    previous = letter
  }
  return anchors
}

/** Un filtre au moins restreint la liste (le tri n'en est pas un). */
export const isFiltered = (f: Filters) =>
  f.q.trim() !== '' || f.read !== null || f.status !== null || f.library !== null || f.tags.length > 0

/** Tags de toutes les séries (une graphie par tag, le mot de la source quand il est connu), avec leur
 * nombre de séries ; à trier par l'appelant, selon la langue d'affichage. */
export function allTags(series: SeriesSummary[]): { tag: string; count: number }[] {
  const byKey = new Map<string, { tag: string; count: number }>()
  for (const s of series) {
    const seen = new Set<string>()
    for (const tag of s.tags) {
      const key = tagKey(tag)
      if (seen.has(key)) continue
      seen.add(key)
      const entry = byKey.get(key)
      if (entry) entry.count++
      else byKey.set(key, { tag: canonicalTag(tag), count: 1 })
    }
  }
  return [...byKey.values()]
}
