import { describe, expect, it } from 'vitest'
import { formatDate, formatDuration } from './format'

// Intl sépare le nombre de son unité par une espace insécable : on compare avec des espaces simples.
const plain = (text: string) => text.replace(/\s/g, ' ')

describe('formats de l’administration', () => {
  it('écrit une durée à l’échelle qui convient', () => {
    expect(plain(formatDuration(850, 'fr'))).toBe('850 ms')
    expect(plain(formatDuration(12_480, 'fr'))).toBe('12,5 s')
    expect(plain(formatDuration(12_480, 'en'))).toBe('12.5 sec')
    expect(plain(formatDuration(185_000, 'fr'))).toBe('3 min 5 s')
  })

  it('écrit une date dans la langue demandée', () => {
    expect(plain(formatDate('2026-11-13T09:05:00.000Z', 'fr'))).toMatch(/13 nov\. 2026/)
    expect(plain(formatDate('2026-11-13T09:05:00.000Z', 'en'))).toMatch(/Nov 13, 2026/)
  })
})
