// Ordre voulu : Edge et Opera se disent aussi « Chrome », Chrome se dit aussi « Safari », Android
// se dit aussi « Linux ».
const BROWSERS: [RegExp, string][] = [
  [/Edg\//, 'Edge'],
  [/OPR\//, 'Opera'],
  [/SamsungBrowser\//, 'Samsung Internet'],
  [/Firefox\/|FxiOS\//, 'Firefox'],
  [/Chrome\/|CriOS\//, 'Chrome'],
  [/Safari\//, 'Safari'],
]
const SYSTEMS: [RegExp, string][] = [
  [/Android/, 'Android'],
  [/iPhone|iPad|iPod/, 'iOS'],
  [/Windows/, 'Windows'],
  [/Mac OS X|Macintosh/, 'macOS'],
  [/Linux/, 'Linux'],
]

/** « Navigateur · Système » d'après le `User-Agent` d'une session ; le texte brut s'il n'y a rien
 * de connu (client maison, application native). */
export function deviceName(userAgent: string): string {
  const find = (list: [RegExp, string][]) => list.find(([pattern]) => pattern.test(userAgent))?.[1]
  const name = [find(BROWSERS), find(SYSTEMS)].filter(Boolean).join(' · ')
  return name || userAgent
}
