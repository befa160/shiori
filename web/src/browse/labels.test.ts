import { describe, expect, it } from 'vitest'
import type { SeriesVolume } from '@/api/types'
import { count, volumeLabel } from './labels'

const volume = (v: Partial<SeriesVolume>): SeriesVolume => ({
  volume_id: 'v',
  kind: 'volume',
  number: null,
  number_end: null,
  status: 'ok',
  page_count: 10,
  cover_page_id: null,
  file_name: 'fichier.cbz',
  ...v,
})

describe('volumeLabel', () => {
  it.each([
    [{ kind: 'volume', number: 3 }, 'Tome 3'],
    [{ kind: 'volume', number: 1, number_end: 3 }, 'Tomes 1–3'],
    [{ kind: 'chapter', number: 12.5 }, 'Chapitre 12,5'],
    [{ kind: 'chapter', number: 10, number_end: 12 }, 'Chapitres 10–12'],
    [{ kind: 'special', number: 2 }, 'Spécial 2'],
    [{ kind: 'special' }, 'Spécial'],
    [{ kind: 'unknown', number: 7 }, 'N° 7'],
    [{ kind: 'unknown' }, 'fichier.cbz'],
    [{ kind: 'volume' }, 'fichier.cbz'],
  ] as const)('%o → %s', (v, label) => {
    expect(volumeLabel(volume(v))).toBe(label)
  })
})

describe('count', () => {
  it('accorde et sépare les milliers à la française', () => {
    expect(count(1, 'tome', 'tomes')).toBe('1 tome')
    expect(count(0, 'tome', 'tomes')).toBe('0 tomes')
    expect(count(12760, 'page', 'pages')).toBe('12 760 pages')
  })
})
