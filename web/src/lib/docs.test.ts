import { describe, expect, it } from 'vitest'
import { docsUrl } from './docs'

describe('lien vers la documentation', () => {
  it('suit la langue de l’interface, français à la racine', () => {
    expect(docsUrl('fr')).toBe('https://befa160.github.io/shiori/')
    expect(docsUrl('en')).toBe('https://befa160.github.io/shiori/en/')
    expect(docsUrl('en-GB', 'guide/diagnostics')).toBe('https://befa160.github.io/shiori/en/guide/diagnostics')
    expect(docsUrl('fr', 'guide/diagnostics')).toBe('https://befa160.github.io/shiori/guide/diagnostics')
  })
})
