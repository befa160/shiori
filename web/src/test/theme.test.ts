/// <reference types="node" />
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// `?raw` sur un CSS donne une chaîne vide sous Vitest : le thème est lu sur le disque.
const css = readFileSync('src/index.css', 'utf8') // Vitest tourne depuis web/

// Une couleur utilisée sans être définie dans le thème rend transparent (bug réel : menu du
// Select sans `popover`). Chaque nouveau composant shadcn est vérifié ici.
const TOKENS =
  /\b(?:bg|text|border|ring|fill|stroke|outline|divide|placeholder)-(?:popover|card|muted|accent|primary|secondary|destructive|border|input|ring|background|foreground)(?:-foreground|-fill)?\b/g

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
