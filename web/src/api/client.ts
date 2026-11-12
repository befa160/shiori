import type { Health, Library, PageInfo, SeriesDetail, SeriesSummary, VolumeDetail } from './types'

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

/** GET JSON ; `null` pour un 204. Toute réponse non 2xx devient une `ApiError`. */
export async function get<T>(path: string, init?: RequestInit): Promise<T | null> {
  let response: Response
  try {
    response = await fetch(`/api${path}`, { ...init, headers: { accept: 'application/json' } })
  } catch {
    throw new ApiError(0, 'network', 'serveur injoignable')
  }
  if (!response.ok) {
    const body = await response.json().catch(() => null)
    const code = typeof body?.code === 'string' ? body.code : 'http_error'
    const message = typeof body?.message === 'string' ? body.message : `erreur HTTP ${response.status}`
    throw new ApiError(response.status, code, message)
  }
  return response.status === 204 ? null : ((await response.json()) as T)
}

/** Comme `get`, pour les routes qui ne répondent jamais 204. */
async function must<T>(path: string, init?: RequestInit): Promise<T> {
  return (await get<T>(path, init)) as T
}

export const api = {
  health: (init?: RequestInit) => must<Health>('/health', init),
  libraries: (init?: RequestInit) => must<Library[]>('/libraries', init),
  librarySeries: (id: number, init?: RequestInit) => must<SeriesSummary[]>(`/libraries/${id}/series`, init),
  series: (id: number, init?: RequestInit) => must<SeriesDetail>(`/series/${id}`, init),
  volume: (id: string, init?: RequestInit) => must<VolumeDetail>(`/volumes/${id}`, init),
  pages: (volumeId: string, init?: RequestInit) => must<PageInfo[]>(`/volumes/${volumeId}/pages`, init),
}

/** Largeurs que le serveur sait produire (backend/crates/shiori/src/render.rs, `WIDTHS`). */
export const WIDTHS = [160, 320, 480, 640, 800, 1080, 1280, 1600, 2048] as const
export type Width = (typeof WIDTHS)[number]
export type ImageFormat = 'avif' | 'jpeg'

/** URL immuable d'une page : originale, ou redimensionnée à une largeur de la liste fixe. */
export function pageImageUrl(pageId: string, resize?: { width: Width; format: ImageFormat }): string {
  const base = `/api/pages/${encodeURIComponent(pageId)}/image`
  return resize ? `${base}?w=${resize.width}&f=${resize.format}` : base
}
