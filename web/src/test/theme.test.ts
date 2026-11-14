/// <reference types="node" />
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// `?raw` sur un CSS donne une chaîne vide sous Vitest : le thème est lu sur le disque.
const css = readFileSync('src/index.css', 'utf8') // Vitest tourne depuis web/

// Une couleur utilisée sans être définie dans le thème rend transparent (bug réel : menu du
// Select sans `popover`). Chaque nouveau composant shadcn est vérifié ici.
const TOKENS =
  /\b(?:bg|text|border|ring|fill|stroke|outline|divide|placeholder)-(?:sidebar-foreground|sidebar-primary|sidebar-accent|sidebar-border|sidebar|popover|card|muted|accent|primary|secondary|destructive|border|input|ring|background|foreground)(?:-foreground|-fill)?\b/g

const sources = import.meta.glob<string>(['../**/*.{ts,tsx}', '!../**/*.test.{ts,tsx}'], {
  query: '?raw',
  import: 'default',
  eager: true,
})

describe('thème', () => {
  it('définit chaque couleur utilisée par les composants', () => {
    const used = new Set(
      Object.values(sources).flatMap((code) => [...code.matchAll(TOKENS)].map((m) => m[0].replace(/^[a-z]+-/, ''))),
    )
    expect(used.size).toBeGreaterThan(5)
    expect([...used].filter((token) => !css.includes(`--color-${token}:`))).toEqual([])
  })
})

/** Jetons d'un bloc `sélecteur { … }` de `index.css`. */
function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`)
  if (start < 0) throw new Error(`bloc ${selector} absent`)
  const body = css.slice(start, css.indexOf('}', start))
  return Object.fromEntries([...body.matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]))
}

// Le sombre est la base (`:root`) ; chaque autre thème redéfinit une partie des jetons.
const DARK = block(":root,\n[data-theme='dark']")
const THEMES = {
  dark: DARK,
  oled: { ...DARK, ...block("[data-theme='oled']") },
  light: { ...DARK, ...block("[data-theme='light']") },
  sepia: { ...DARK, ...block("[data-theme='sepia']") },
}

function luminance(hex: string) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function contrast(a: string, b: string) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

// Couples réellement affichés : texte 4,5:1, bordure de champ et anneau de focus 3:1 (WCAG AA).
const TEXT: [string, string][] = [
  ['foreground', 'background'],
  ['foreground', 'card'],
  ['card-foreground', 'card'],
  ['popover-foreground', 'popover'],
  ['muted-foreground', 'background'],
  ['muted-foreground', 'card'],
  ['muted-foreground', 'muted'],
  ['accent-foreground', 'accent'],
  ['secondary-foreground', 'secondary'],
  ['primary-foreground', 'primary'],
  ['primary', 'background'],
  ['primary', 'card'],
  ['destructive', 'background'],
  ['destructive', 'card'],
  ['destructive', 'muted'],
]
const NON_TEXT: [string, string][] = [
  ['input', 'background'],
  ['input', 'card'],
  ['ring', 'background'],
  ['ring', 'card'],
]

describe.each(Object.entries(THEMES))('thème %s', (_name, tokens) => {
  it.each(TEXT)('texte %s sur %s : 4,5:1 au moins', (fg, bg) => {
    expect(contrast(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(4.5)
  })
  it.each(NON_TEXT)('%s sur %s : 3:1 au moins', (fg, bg) => {
    expect(contrast(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(3)
  })
  it('bouton destructif : texte blanc 4,5:1 au moins', () => {
    expect(contrast('#ffffff', tokens['destructive-fill'])).toBeGreaterThanOrEqual(4.5)
  })
})
