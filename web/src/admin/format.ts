/** Durée lisible : « 850 ms », « 12,5 s », « 3 min 05 s », dans la langue de l'interface. */
export function formatDuration(ms: number, language: string): string {
  const unit = (value: number, u: 'millisecond' | 'second' | 'minute', digits = 0) =>
    new Intl.NumberFormat(language, { style: 'unit', unit: u, unitDisplay: 'short', maximumFractionDigits: digits }).format(value)
  if (ms < 1000) return unit(ms, 'millisecond')
  if (ms < 60_000) return unit(ms / 1000, 'second', 1)
  const minutes = Math.floor(ms / 60_000)
  const seconds = Math.round((ms % 60_000) / 1000)
  return `${unit(minutes, 'minute')} ${unit(seconds, 'second')}`
}

/** Date et heure courtes, dans la langue et le fuseau de l'appareil. */
export function formatDate(iso: string, language: string): string {
  return new Intl.DateTimeFormat(language, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso))
}
