// Réponses JSON de `shiori serve` (backend/crates/shiori-db/src/browse.rs, server.rs).
// Le lecteur n'en déduit rien d'autre que ce qui y est écrit (principe 3).

export type Status = 'ok' | 'warning' | 'error'
export type PageStatus = 'ok' | 'broken'
export type NameKind = 'volume' | 'chapter' | 'special' | 'unknown'
export type Severity = 'info' | 'warning' | 'error'

export type LibraryKind = 'manga' | 'comics' | 'webtoon'
/** `rtl` manga, `ltr` BD, `vertical` webtoon. */
export type ReadingMode = 'rtl' | 'ltr' | 'vertical'
/** `auto` : double en paysage, simple en portrait. */
export type PageLayout = 'single' | 'double' | 'auto'
/** Calculé par le serveur d'après la progression des tomes de la série. */
export type ReadingStatus = 'unread' | 'in_progress' | 'read'
export type PublicationStatus = 'ongoing' | 'completed' | 'hiatus' | 'cancelled'

/** Sens des taps, du balayage, des flèches et du curseur, séparé du sens de lecture (qui garde
 * l'ordre des doubles pages) : `reading` comme lui, `ltr` toucher à droite avance, `rtl` à gauche. */
export type PageTurn = 'reading' | 'ltr' | 'rtl'

/** Réglages appliqués, et ce que la série impose elle-même (`null` = défaut de la bibliothèque,
 * ou défaut global pour `page_turn`). */
export interface ReadingSettings {
  reading_mode: ReadingMode
  page_layout: PageLayout
  page_turn: PageTurn
  reading_mode_override: ReadingMode | null
  page_layout_override: PageLayout | null
  page_turn_override: PageTurn | null
}

/** Réglages qu'une série impose ; champ absent = inchangé, `null` = retour au défaut. */
export type SettingsPatch = Partial<Pick<ReadingSettings, 'reading_mode' | 'page_layout'>> & {
  page_turn?: PageTurn | null
}

export interface Progress {
  page_id: string
  ordinal: number
  page_count: number
  /** La dernière page a été atteinte au moins une fois. */
  finished: boolean
  updated_at: string
}

export type Theme = 'system' | 'dark' | 'light' | 'oled' | 'sepia'

export interface Settings {
  /** `null` : personne n'a choisi, l'interface suit la langue du navigateur. */
  language: 'fr' | 'en' | null
  /** Sens par défaut pour tourner les pages ; une série peut avoir le sien. */
  page_turn: PageTurn
  /** Métadonnées web autorisées (désactivées par défaut). */
  metadata_online: boolean
  /** Ordre choisi des sources ; `null` : d'après la langue de l'interface. */
  metadata_sources: SourceId[] | null
  /** Fiches pour adultes permises dans les recherches (exclues par défaut). */
  metadata_adult: boolean
  /** Thème de l'interface ; `system` suit l'appareil (sombre ou clair). Le lecteur reste sombre. */
  theme: Theme
}

/** Sources de métadonnées web. */
export type SourceId = 'anilist' | 'mangadex'

/** Un texte et sa langue (`en`, `fr`, `ja`, `ja-ro`…). */
export interface LangText {
  lang: string | null
  text: string
}

/** Fiche d'une série chez une source (candidat ou résultat de recherche). */
export interface WebMetadata {
  id: string
  /** Titre principal d'abord. */
  titles: LangText[]
  summaries: LangText[]
  authors: Author[]
  genres: string[]
  tags: string[]
  year: number | null
  status: string | null
  links: string[]
}

/** État d'une source pour une série. */
export interface SourceState {
  source: SourceId
  /** Fiche liée ; `null` : pas liée. */
  external_id: string | null
  matched_by: 'auto' | 'user' | null
  checked_at: string
  last_error: string | null
  /** Fiches possibles, à confirmer ; vide : rien trouvé ou liée. */
  candidates: WebMetadata[]
}

/** Une source qui demande l'attention : fiches à confirmer, ou dernier essai raté. */
export interface SourceIssue {
  series_id: number
  series_name: string
  source: SourceId
  candidates: number
  /** Code stable (`offline`, `rate_limited`, `not_found`, `http_error`, `bad_response`). */
  last_error: string | null
}

/** `GET /api/admin/metadata`. */
export interface MetadataOverview {
  online: boolean
  /** Ordre appliqué des sources. */
  sources: SourceId[]
  identify: { running: boolean; done: number; total: number; stopped: string | null }
  issues: SourceIssue[]
}

export interface Health {
  status: 'ok'
  version: string
}

export interface Library {
  id: number
  name: string
  root: string
  kind: LibraryKind
  series_count: number
  volume_count: number
}

export interface SeriesSummary {
  id: number
  library_id: number
  /** Titre affiché : édité, sinon celui des fichiers, sinon le nom tiré du dossier. */
  name: string
  /** Pour la recherche : autres titres et noms des auteurs. */
  alt_titles: string[]
  authors: string[]
  volume_count: number
  /** Tomes lus jusqu'au bout. */
  read_count: number
  /** Pages de tous les tomes, et pages lues (tome fini en entier, tome en cours jusqu'à la page). */
  page_count: number
  pages_read: number
  reading_status: ReadingStatus
  /** `null` : inconnu. */
  publication_status: PublicationStatus | null
  created_at: string
  last_read_at: string | null
  /** Page choisie comme couverture, sinon page 0 du premier tome. */
  cover_page_id: string | null
  /** Couverture d'une source choisie (`/api/covers/{id}`) : l'emporte sur la page. */
  cover_image_id: string | null
  /** Tome qu'ouvre « Lire » : la suite de la lecture, sinon le premier non lu, sinon le premier. */
  resume_volume_id: string | null
}

/** Une entrée de « Continuer la lecture » (`GET /api/continue`), une par série. */
export interface ContinueItem {
  series_id: number
  series_name: string
  library_id: number
  /** Tome en cours (avec `progress`) ou tome suivant (sans). */
  volume: SeriesVolume
  updated_at: string
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
  /** Absent si le tome n'a jamais été ouvert. */
  progress?: Progress
}

/** Ni scénario ni dessin : rôle inconnu. */
export interface Author {
  name: string
  story: boolean
  art: boolean
}

/** Métadonnées fusionnées d'une série (fichiers, puis éditions à la main). */
export interface SeriesMetadata {
  title: string | null
  alt_titles: string[]
  summary: string | null
  authors: Author[]
  genres: string[]
  tags: string[]
  year: number | null
  publisher: string | null
  age_rating: string | null
  publication_status: PublicationStatus | null
  links: string[]
  cover_page_id: string | null
  cover_image_id: string | null
  /** Source qui passe en premier pour cette série ; `null` : l'ordre réglé. */
  main_source: SourceId | null
  /** Sens de lecture proposé par les fichiers : une suggestion, jamais imposée. */
  right_to_left: boolean | null
}

/** Champs éditables ; `right_to_left` vient seulement des fichiers. */
export type SeriesField = Exclude<keyof SeriesMetadata, 'right_to_left'>

/** Champ donné = édité et verrouillé (`null` compris) ; `unlock` le rend aux sources. */
export type SeriesPatch = Partial<Omit<SeriesMetadata, 'right_to_left'>> & { unlock?: SeriesField[] }

export interface SeriesDetail {
  id: number
  library_id: number
  name: string
  folder: string | null
  metadata: SeriesMetadata
  /** Champs édités à la main, que les sources ne remplacent plus. */
  locked: SeriesField[]
  /** Sources web de la série : liées, à confirmer ou en échec. */
  sources: SourceState[]
  publication_status: PublicationStatus | null
  settings: ReadingSettings
  /** Tome qu'ouvre « Lire » (même règle que dans `SeriesSummary`). */
  resume_volume_id: string | null
  /** Comme dans `SeriesSummary`. */
  page_count: number
  pages_read: number
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
  settings: ReadingSettings
  progress: Progress | null
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

// Scans (backend/crates/shiori-db/src/runs.rs, shiori/src/job.rs).
export type RunStatus = 'running' | 'completed' | 'cancelled' | 'failed' | 'interrupted'

export interface RunCounts {
  ok: number
  warning: number
  error: number
  unchanged: number
  removed: number
}

/** Passe de scan en cours (`GET /api/admin/scans/current`). */
export interface ScanProgress {
  run: number
  library_id: number
  target: string
  /** 0 tant que la liste des fichiers n'est pas faite. */
  total: number
  done: number
  current: string | null
  counts: RunCounts
  cancelling: boolean
}

/** Une passe de l'historique (`GET /api/admin/scans`, du plus récent au plus ancien). */
export interface RunSummary {
  id: number
  library_id: number | null
  origin: 'cli' | 'api'
  status: RunStatus
  root: string
  options: { deep?: boolean; rehash?: boolean }
  started_at: string
  finished_at: string | null
  duration_ms: number | null
  files_total: number | null
  counts: RunCounts
  /** Erreur fatale qui a arrêté la passe. */
  error: string | null
}

export interface LogLine {
  id: number
  at: string
  level: Severity
  path: string | null
  code: string | null
  message: string
  /** Série et tome de ce chemin aujourd'hui ; `null` si le fichier n'est plus suivi. */
  series_id: number | null
  volume_id: string | null
}
