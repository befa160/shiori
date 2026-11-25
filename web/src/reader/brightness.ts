// Luminosité du lecteur (lot 1.2a) : retenue par appareil (Q68), pas par compte, le soir sur la
// tablette n'étant pas le jour sur le téléphone.
import { useState } from 'react'

const STORAGE_KEY = 'shiori.brightness'
/** En dessous, la page devient illisible : 30 % reste le plus sombre. */
export const MIN_BRIGHTNESS = 30

/** Valeur gardée sur l'appareil, en pourcentage ; 100 sans valeur valable. */
export function storedBrightness(): number {
  let stored: string | null = null
  try {
    stored = localStorage.getItem(STORAGE_KEY)
  } catch {
    // Stockage refusé : pleine luminosité.
  }
  const value = Number(stored)
  return stored !== null && Number.isFinite(value) ? clamp(value) : 100
}

function clamp(value: number) {
  return Math.min(100, Math.max(MIN_BRIGHTNESS, Math.round(value)))
}

export function useBrightness(): [number, (value: number) => void] {
  const [brightness, setBrightness] = useState(storedBrightness)
  const change = (value: number) => {
    const next = clamp(value)
    setBrightness(next)
    try {
      localStorage.setItem(STORAGE_KEY, String(next))
    } catch {
      // idem : le réglage vaut pour la séance.
    }
  }
  return [brightness, change]
}
