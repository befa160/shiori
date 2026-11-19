// Progression lue sans réseau (lot 6.5b) : gardée sur l'appareil, une page par tome (la dernière),
// puis envoyée au serveur au retour du réseau ou au prochain lancement. Une file par compte
// (lot 7.7) : elle n'est envoyée qu'avec la session du compte qui l'a remplie.
import { ApiError, api } from '@/api/client'

/** Avant les comptes : une seule file, sans propriétaire. */
const LEGACY = 'shiori.pendingProgress'
/** Dernier compte connecté sur cet appareil : sans réseau, la garde ne peut pas le demander. */
const USER = 'shiori.user'

function get(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function set(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key)
    else localStorage.setItem(key, value)
  } catch {
    // Stockage refusé : la page lue hors ligne sera perdue, comme avant le lot 6.5b.
  }
}

/** Sans compte connu (première ouverture hors ligne de cette version, ou après une déconnexion) :
 * la file sans propriétaire, qui revient au prochain compte connecté. */
function key(): string {
  const user = get(USER)
  return user ? `${LEGACY}.${user}` : LEGACY
}

/** Page lue et son heure ; une simple chaîne vient d'une version sans l'heure. */
type Pending = { page: string; at?: string }
type Queue = Record<string, Pending | string>

function read(): Queue {
  const k = key()
  try {
    return JSON.parse(get(k) ?? '{}') || {}
  } catch {
    return {}
  }
}

const entry = (value: Pending | string): Pending => (typeof value === 'string' ? { page: value } : value)

function write(pending: Queue) {
  set(key(), Object.keys(pending).length ? JSON.stringify(pending) : null)
}

/** Compte connecté (`null` à la déconnexion) ; envoie ce qu'il a lu hors ligne. La file d'avant
 * les comptes revient au premier qui se connecte sur l'appareil. */
export function setProgressUser(id: number | null) {
  set(USER, id === null ? null : String(id))
  if (id === null) return
  const legacy = get(LEGACY)
  if (legacy) {
    try {
      write({ ...JSON.parse(legacy), ...read() })
    } catch {
      // File illisible : abandonnée.
    }
    set(LEGACY, null)
  }
  void flushProgress()
}

export function queueProgress(volumeId: string, pageId: string) {
  write({ ...read(), [volumeId]: { page: pageId, at: new Date().toISOString() } })
}

/** Page lue hors ligne et pas encore envoyée, pour reprendre au bon endroit sans réseau. */
export function pendingPage(volumeId: string): string | undefined {
  const value = read()[volumeId]
  return value === undefined ? undefined : entry(value).page
}

/** Coupure passagère, à réessayer plus tard : réseau absent, session expirée (401), proxy ou
 * serveur injoignable (502 et plus). Le reste est une vraie réponse du serveur. */
export function retryable(error: unknown) {
  return !(error instanceof ApiError) || error.status === 0 || error.status === 401 || error.status >= 502
}

/** Envoie ce qui attend ; s'arrête à la première coupure, abandonne ce que le serveur refuse. */
export async function flushProgress() {
  for (const [volumeId, value] of Object.entries(read())) {
    const { page, at } = entry(value)
    try {
      // Avec son heure : une page lue depuis sur un autre appareil n'est pas écrasée.
      await api.saveProgress(volumeId, page, undefined, at)
    } catch (error) {
      if (retryable(error)) return
    }
    const { [volumeId]: _sent, ...rest } = read()
    write(rest)
  }
}
