import type {
  BackupFile,
  Backups,
  Account,
  AccountPatch,
  ApiToken,
  AuthState,
  ContinueItem,
  NewVolume,
  Device,
  Enrollment,
  LoginResult,
  Me,
  Preferences,
  Role,
  User,
  FileHealth,
  Health,
  LibrariesHealth,
  Library,
  LibraryKind,
  LogLine,
  MetadataOverview,
  PageInfo,
  Progress,
  ReadingStats,
  ReadingSettings,
  RunSummary,
  ScanProgress,
  SeriesDetail,
  SeriesPatch,
  SeriesSummary,
  SourceId,
  StoragePaths,
  WebMetadata,
  Settings,
  SettingsPatch,
  VolumeDetail,
  VolumePatch,
  Bookmark,
} from './types'

/** Erreur de l'API : code stable (`page_broken`, `volume_changed`…) et message en français. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

/** Requête JSON ; `null` pour une réponse sans corps (204, 202). Toute réponse non 2xx devient
 * une `ApiError`. */
export async function request<T>(path: string, init?: RequestInit): Promise<T | null> {
  let response: Response
  try {
    // Corps texte : du JSON ; un fichier garde son propre type.
    const headers = { accept: 'application/json', ...(typeof init?.body === 'string' ? { 'content-type': 'application/json' } : {}) }
    response = await fetch(`/api${path}`, { ...init, headers })
  } catch {
    throw new ApiError(0, 'network', 'serveur injoignable')
  }
  if (!response.ok) {
    const body = await response.json().catch(() => null)
    const code = typeof body?.code === 'string' ? body.code : 'http_error'
    const message = typeof body?.message === 'string' ? body.message : `erreur HTTP ${response.status}`
    throw new ApiError(response.status, code, message)
  }
  const text = await response.text()
  return text ? (JSON.parse(text) as T) : null
}

/** Comme `request`, pour les routes qui ne répondent jamais 204. */
async function must<T>(path: string, init?: RequestInit): Promise<T> {
  return (await request<T>(path, init)) as T
}

export const api = {
  health: (init?: RequestInit) => must<Health>('/health', init),
  settings: (init?: RequestInit) => must<Settings>('/settings', init),
  saveSettings: (settings: Partial<Settings>) =>
    must<Settings>('/settings', { method: 'PUT', body: JSON.stringify(settings) }),
  libraries: (init?: RequestInit) => must<Library[]>('/libraries', init),
  librarySeries: (id: number, init?: RequestInit) => must<SeriesSummary[]>(`/libraries/${id}/series`, init),
  allSeries: (init?: RequestInit) => must<SeriesSummary[]>('/series', init),
  continueReading: (init?: RequestInit) => must<ContinueItem[]>('/continue', init),
  newVolumes: (init?: RequestInit) => must<NewVolume[]>('/me/new', init),
  markNewSeen: () => request<null>('/me/new/seen', { method: 'POST' }),
  follow: (seriesId: number, on: boolean) => request<null>(`/series/${seriesId}/follow`, { method: on ? 'PUT' : 'DELETE' }),
  /** Statistiques du compte sur `days` jours, « aujourd'hui » étant celui de l'appareil. */
  readingStats: (days: number, init?: RequestInit) =>
    must<ReadingStats>(`/me/stats?days=${days}&utc_offset=${-new Date().getTimezoneOffset()}`, init),
  series: (id: number, init?: RequestInit) => must<SeriesDetail>(`/series/${id}`, init),
  volume: (id: string, init?: RequestInit) => must<VolumeDetail>(`/volumes/${id}`, init),
  pages: (volumeId: string, init?: RequestInit) => must<PageInfo[]>(`/volumes/${volumeId}/pages`, init),
  /** `readAt` : heure de lecture d'une page lue hors ligne ; le serveur garde la plus récente.
   * `utc_offset` : fuseau de l'appareil, qui donne le jour compté par les statistiques. */
  saveProgress: (volumeId: string, pageId: string, init?: RequestInit, readAt?: string) =>
    must<Progress>(`/volumes/${volumeId}/progress`, {
      ...init,
      method: 'PUT',
      body: JSON.stringify({ page_id: pageId, read_at: readAt, utc_offset: -new Date().getTimezoneOffset() }),
    }),
  /** « Marquer comme lu » : la dernière page, hors des statistiques (ce n'est pas une lecture). */
  markVolumeRead: (volumeId: string, lastPageId: string) =>
    must<Progress>(`/volumes/${volumeId}/progress`, { method: 'PUT', body: JSON.stringify({ page_id: lastPageId, stats: false }) }),
  createLibrary: (library: { name: string; root: string; kind: LibraryKind; share: boolean }) =>
    must<Library>('/libraries', { method: 'POST', body: JSON.stringify(library) }),
  setLibraryWatch: (id: number, watch: boolean) =>
    request<null>(`/libraries/${id}`, { method: 'PATCH', body: JSON.stringify({ watch }) }),
  deleteLibrary: (id: number) => request<null>(`/libraries/${id}`, { method: 'DELETE' }),
  startScan: (libraryId: number, options: { deep?: boolean; rehash?: boolean } = {}) =>
    must<{ run: number }>('/admin/scans', { method: 'POST', body: JSON.stringify({ library_id: libraryId, ...options }) }),
  currentScan: (init?: RequestInit) => request<ScanProgress>('/admin/scans/current', init),
  cancelScan: () => request<null>('/admin/scans/current/cancel', { method: 'POST' }),
  librariesHealth: (init?: RequestInit) => must<LibrariesHealth>('/admin/health', init),
  filesToRepair: (query: URLSearchParams, init?: RequestInit) => must<FileHealth[]>(`/admin/health/files?${query}`, init),
  scans: (query: URLSearchParams, init?: RequestInit) => must<RunSummary[]>(`/admin/scans?${query}`, init),
  scan: (id: number, init?: RequestInit) => must<RunSummary>(`/admin/scans/${id}`, init),
  scanLogs: (id: number, query: URLSearchParams, init?: RequestInit) =>
    must<LogLine[]>(`/admin/scans/${id}/logs?${query}`, init),
  clearProgress: (volumeId: string) => request<null>(`/volumes/${volumeId}/progress`, { method: 'DELETE' }),
  /** Toute la série lue (dernière page de chaque tome) ou non lue (progression effacée). */
  markSeries: (seriesId: number, read: boolean) =>
    must<SeriesDetail>(`/series/${seriesId}/read`, { method: read ? 'PUT' : 'DELETE' }),
  bookmarks: (volumeId: string, init?: RequestInit) => must<Bookmark[]>(`/volumes/${volumeId}/bookmarks`, init),
  seriesBookmarks: (seriesId: number, init?: RequestInit) => must<Bookmark[]>(`/series/${seriesId}/bookmarks`, init),
  addBookmark: (volumeId: string, pageId: string, note = '') =>
    must<Bookmark>(`/volumes/${volumeId}/bookmarks`, { method: 'POST', body: JSON.stringify({ page_id: pageId, note }) }),
  setBookmarkNote: (id: number, note: string) => must<Bookmark>(`/bookmarks/${id}`, { method: 'PATCH', body: JSON.stringify({ note }) }),
  deleteBookmark: (id: number) => request<null>(`/bookmarks/${id}`, { method: 'DELETE' }),
  patchVolume: (volumeId: string, patch: VolumePatch) =>
    must<VolumeDetail>(`/volumes/${volumeId}`, { method: 'PATCH', body: JSON.stringify(patch) }),
  patchSeries: (seriesId: number, patch: SeriesPatch) =>
    must<SeriesDetail>(`/series/${seriesId}`, { method: 'PATCH', body: JSON.stringify(patch) }),
  searchMetadata: (seriesId: number, source: SourceId, q: string, init?: RequestInit) =>
    must<WebMetadata[]>(`/series/${seriesId}/metadata/search?${new URLSearchParams({ source, q })}`, init),
  linkSource: (seriesId: number, source: SourceId, externalId: string) =>
    must<SeriesDetail>(`/series/${seriesId}/metadata/links/${source}`, {
      method: 'PUT',
      body: JSON.stringify({ external_id: externalId }),
    }),
  unlinkSource: (seriesId: number, source: SourceId) =>
    must<SeriesDetail>(`/series/${seriesId}/metadata/links/${source}`, { method: 'DELETE' }),
  refreshMetadata: (seriesId: number) => must<SeriesDetail>(`/series/${seriesId}/metadata/refresh`, { method: 'POST' }),
  useSourceCover: (seriesId: number, source: SourceId) =>
    must<SeriesDetail>(`/series/${seriesId}/metadata/cover`, { method: 'POST', body: JSON.stringify({ source }) }),
  metadataOverview: (init?: RequestInit) => must<MetadataOverview>('/admin/metadata', init),
  storage: (init?: RequestInit) => must<StoragePaths>('/admin/storage', init),
  backups: (init?: RequestInit) => must<Backups>('/admin/backups', init),
  createBackup: () => must<BackupFile>('/admin/backups', { method: 'POST' }),
  uploadBackup: (file: File) => must<BackupFile>('/admin/backups/upload', { method: 'POST', body: file }),
  restoreBackup: (name: string) => request<null>(`/admin/backups/${encodeURIComponent(name)}/restore`, { method: 'POST' }),
  deleteBackup: (name: string) => request<null>(`/admin/backups/${encodeURIComponent(name)}`, { method: 'DELETE' }),
  identify: () => request<null>('/admin/metadata/identify', { method: 'POST' }),
  authState: (init?: RequestInit) => must<AuthState>('/auth/state', init),
  setup: (body: { token: string; username: string; password: string }) =>
    must<{ user: User }>('/auth/setup', { method: 'POST', body: JSON.stringify(body) }),
  login: (body: { username: string; password: string }) =>
    must<LoginResult>('/auth/login', { method: 'POST', body: JSON.stringify(body) }),
  secondFactor: (challenge: string, code: string) =>
    must<{ user: User }>('/auth/totp', { method: 'POST', body: JSON.stringify({ challenge, code }) }),
  startTotp: (password: string) => must<Enrollment>('/me/totp', { method: 'POST', body: JSON.stringify({ password }) }),
  confirmTotp: (code: string) =>
    must<{ recovery_codes: string[] }>('/me/totp/confirm', { method: 'POST', body: JSON.stringify({ code }) }),
  disableTotp: (password: string, code: string) =>
    request<null>('/me/totp', { method: 'DELETE', body: JSON.stringify({ password, code }) }),
  tokens: (init?: RequestInit) => must<ApiToken[]>('/me/tokens', init),
  createToken: (body: { name: string; expires_in_days?: number; password: string }) =>
    must<ApiToken & { token: string }>('/me/tokens', { method: 'POST', body: JSON.stringify(body) }),
  revokeToken: (id: number) => request<null>(`/me/tokens/${id}`, { method: 'DELETE' }),
  resetTotp: (id: number) => request<null>(`/admin/users/${id}/totp`, { method: 'DELETE' }),
  logout: () => request<null>('/auth/logout', { method: 'POST' }),
  me: (init?: RequestInit) => must<Me>('/me', init),
  saveMe: (patch: Partial<Preferences>) => must<Me>('/me', { method: 'PUT', body: JSON.stringify(patch) }),
  changePassword: (current: string, next: string) =>
    request<null>('/me/password', { method: 'POST', body: JSON.stringify({ current, new: next }) }),
  devices: (init?: RequestInit) => must<Device[]>('/me/sessions', init),
  revokeDevice: (id: number) => request<null>(`/me/sessions/${id}`, { method: 'DELETE' }),
  accounts: (init?: RequestInit) => must<Account[]>('/admin/users', init),
  createAccount: (body: { username: string; role: Role; libraries?: number[] }) =>
    must<{ user: Account; password: string }>('/admin/users', { method: 'POST', body: JSON.stringify(body) }),
  updateAccount: (id: number, patch: AccountPatch) =>
    must<Account>(`/admin/users/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }),
  deleteAccount: (id: number) => request<null>(`/admin/users/${id}`, { method: 'DELETE' }),
  resetPassword: (id: number) => must<{ password: string }>(`/admin/users/${id}/password`, { method: 'POST' }),
  signOutAccount: (id: number) => request<null>(`/admin/users/${id}/sessions`, { method: 'DELETE' }),
  saveSeriesSettings: (seriesId: number, patch: SettingsPatch) =>
    must<ReadingSettings>(`/series/${seriesId}/settings`, { method: 'PATCH', body: JSON.stringify(patch) }),
}

/** Largeurs que le serveur sait produire (backend/crates/shiori/src/render.rs, `WIDTHS`). */
export const WIDTHS = [160, 320, 480, 640, 800, 1080, 1280, 1600, 2048] as const
export type Width = (typeof WIDTHS)[number]
export type ImageFormat = 'avif' | 'jpeg'

/** Vignette d'une fiche, relayée par le serveur (le navigateur ne contacte jamais les sources). */
export function thumbnailUrl(source: SourceId, id: string): string {
  return `/api/metadata/thumbnail/${source}/${encodeURIComponent(id)}`
}

/** URL immuable d'une couverture gardée par le serveur, originale ou redimensionnée. */
export function coverImageUrl(id: string, resize?: { width: Width; format: ImageFormat }): string {
  const base = `/api/covers/${encodeURIComponent(id)}`
  return resize ? `${base}?w=${resize.width}&f=${resize.format}` : base
}

/** URL immuable d'une page : originale, ou redimensionnée à une largeur de la liste fixe. */
export function pageImageUrl(pageId: string, resize?: { width: Width; format: ImageFormat }): string {
  const base = `/api/pages/${encodeURIComponent(pageId)}/image`
  return resize ? `${base}?w=${resize.width}&f=${resize.format}` : base
}
