// Réponses JSON de `shiori serve` (backend/crates/shiori-db/src/browse.rs, server.rs).
// Le lecteur n'en déduit rien d'autre que ce qui y est écrit (principe 3).

export type Status = 'ok' | 'warning' | 'error'
export type PageStatus = 'ok' | 'broken'
export type NameKind = 'volume' | 'chapter' | 'special' | 'unknown'
export type Severity = 'info' | 'warning' | 'error'

export interface Health {
  status: 'ok'
  version: string
}

export interface Library {
  id: number
  name: string
  root: string
  series_count: number
  volume_count: number
}

export interface SeriesSummary {
  id: number
  name: string
  volume_count: number
  cover_page_id: string | null
}

export interface SeriesVolume {
  volume_id: string
  kind: NameKind
  number: number | null
  number_end: number | null
  status: Status
  page_count: number
  cover_page_id: string | null
  file_name: string
}

export interface SeriesDetail {
  id: number
  library_id: number
  name: string
  folder: string | null
  /** Déjà dans l'ordre de lecture de la série. */
  volumes: SeriesVolume[]
}

export interface Issue {
  code: string
  severity: Severity
  entry: string | null
  message: string
}

export interface VolumeDetail {
  id: string
  size: number
  format: 'cbz' | 'cbr'
  status: Status
  page_count: number
  scanned_at: string
  locations: { path: string; library_id: number | null; series_id: number | null }[]
  issues: Issue[]
}

export interface PageInfo {
  id: string
  /** Position de lecture, 0..n-1 : seule source de l'ordre des pages. */
  ordinal: number
  width: number | null
  height: number | null
  media_type: string | null
  status: PageStatus
}
