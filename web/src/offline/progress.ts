// Progression lue sans réseau (lot 6.5b) : gardée sur l'appareil, une page par tome (la dernière),
// puis envoyée au serveur au retour du réseau ou au prochain lancement.
import { ApiError, api } from '@/api/client'

const KEY = 'shiori.pendingProgress'

function read(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? '{}') ?? {}
  } catch {
    return {}
  }
}

function write(pending: Record<string, string>) {
  try {
    localStorage.setItem(KEY, JSON.stringify(pending))
  } catch {
    // Stockage refusé : la page lue hors ligne sera perdue, comme avant ce lot.
  }
}

export function queueProgress(volumeId: string, pageId: string) {
  write({ ...read(), [volumeId]: pageId })
}

/** Page lue hors ligne et pas encore envoyée, pour reprendre au bon endroit sans réseau. */
export function pendingPage(volumeId: string): string | undefined {
  return read()[volumeId]
}

/** Coupure passagère, à réessayer plus tard : réseau absent, session expirée au reverse proxy
 * (401), proxy ou serveur injoignable (502 et plus). Le reste est une vraie réponse du serveur. */
export function retryable(error: unknown) {
  return !(error instanceof ApiError) || error.status === 0 || error.status === 401 || error.status >= 502
}

/** Envoie ce qui attend ; s'arrête à la première coupure, abandonne ce que le serveur refuse. */
export async function flushProgress() {
  for (const [volumeId, pageId] of Object.entries(read())) {
    try {
      await api.saveProgress(volumeId, pageId)
    } catch (error) {
      if (retryable(error)) return
    }
    const { [volumeId]: _sent, ...rest } = read()
    write(rest)
  }
}
