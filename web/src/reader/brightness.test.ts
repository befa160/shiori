import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { storedBrightness, useBrightness } from './brightness'

afterEach(() => vi.restoreAllMocks())

describe('luminosité gardée sur l’appareil', () => {
  it('pleine sans valeur, ou avec une valeur illisible', () => {
    expect(storedBrightness()).toBe(100)
    localStorage.setItem('shiori.brightness', 'nuit')
    expect(storedBrightness()).toBe(100)
  })

  it('bornée entre 30 et 100 %', () => {
    localStorage.setItem('shiori.brightness', '250')
    expect(storedBrightness()).toBe(100)
    localStorage.setItem('shiori.brightness', '64.6')
    expect(storedBrightness()).toBe(65)
  })

  it('pleine si le stockage est refusé', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('refusé')
    })
    expect(storedBrightness()).toBe(100)
  })

  it('vaut pour la séance si le stockage refuse de l’écrire', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('refusé')
    })
    const { result } = renderHook(() => useBrightness())
    act(() => result.current[1](10))
    expect(result.current[0]).toBe(30)
  })
})
