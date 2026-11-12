/** Teinte stable tirée du titre : la même série garde sa couleur, deux voisines diffèrent. */
export function titleHue(title: string): number {
  let hash = 0
  for (const char of title) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) >>> 0
  return hash % 360
}
