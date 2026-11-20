import { describe, expect, it } from 'vitest'
import { canonicalTag, tagLabel, tagSpellings } from './tags'

describe('tags des sources', () => {
  it('traduit le vocabulaire des sources et laisse le reste tel quel', () => {
    expect(tagLabel('Time Skip', 'fr')).toBe('Ellipse temporelle')
    expect(tagLabel('Time Skip', 'en')).toBe('Time Skip')
    // Un terme japonais d'usage courant, ou un tag inconnu, reste tel quel.
    expect(tagLabel('Shounen', 'fr')).toBe('Shounen')
    expect(tagLabel('Mon tag', 'fr')).toBe('Mon tag')
  })

  it('ramène un tag déjà traduit à son mot de source', () => {
    expect(canonicalTag('comédie')).toBe('Comedy')
    expect(tagLabel('Comédie', 'en')).toBe('Comedy')
    expect(tagSpellings('Cooking')).toEqual(['Cooking', 'Cuisine'])
    expect(tagSpellings('Shounen')).toEqual(['Shounen'])
  })
})
