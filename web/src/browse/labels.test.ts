import { describe, expect, it } from 'vitest'
import type { SeriesVolume } from '@/api/types'
import i18n from '@/i18n'
import { volumeLabel } from './labels'

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
    [{ kind: 'volume', number: 3 }, 'Tome 3', 'Volume 3'],
    [{ kind: 'volume', number: 1, number_end: 3 }, 'Tomes 1–3', 'Volumes 1–3'],
    [{ kind: 'chapter', number: 12.5 }, 'Chapitre 12,5', 'Chapter 12.5'],
    [{ kind: 'chapter', number: 10, number_end: 12 }, 'Chapitres 10–12', 'Chapters 10–12'],
    [{ kind: 'special', number: 2 }, 'Spécial 2', 'Special 2'],
    [{ kind: 'special' }, 'Spécial', 'Special'],
    [{ kind: 'unknown', number: 7 }, 'N° 7', 'No. 7'],
    [{ kind: 'unknown' }, 'fichier.cbz', 'fichier.cbz'],
    [{ kind: 'volume' }, 'fichier.cbz', 'fichier.cbz'],
  ] as const)('%o → %s / %s', (v, fr, en) => {
    expect(volumeLabel(volume(v), i18n.getFixedT('fr'), 'fr')).toBe(fr)
    expect(volumeLabel(volume(v), i18n.getFixedT('en'), 'en')).toBe(en)
  })
})

describe('pluriels et nombres', () => {
  it('suivent les règles de chaque langue', () => {
    const fr = i18n.getFixedT('fr')
    const en = i18n.getFixedT('en')
    // En français, 0 et 1 sont au singulier ; en anglais, seul 1 l'est.
    expect([0, 1, 2].map((count) => fr('counts.volumes', { count }))).toEqual(['0 tome', '1 tome', '2 tomes'])
    expect([0, 1, 2].map((count) => en('counts.volumes', { count }))).toEqual(['0 volumes', '1 volume', '2 volumes'])
    expect(fr('counts.pages', { count: 12760 })).toBe('12 760 pages')
    expect(en('counts.pages', { count: 12760 })).toBe('12,760 pages')
  })
})
