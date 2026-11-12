import { describe, expect, it } from 'vitest'
import { fittedWidth, keyMove, moveIndex, pageUrl, requestWidth, swipeAction, tapAction } from './controls'

describe('fittedWidth', () => {
  it('ajuste la page à l’écran sans la déformer', () => {
    // Page portrait 600×900 sur un écran 1000×600 : la hauteur limite (400 de large).
    expect(fittedWidth(1000, 600, { width: 600, height: 900 })).toBe(400)
    // Sur un téléphone 390×844 : la largeur limite.
    expect(fittedWidth(390, 844, { width: 600, height: 900 })).toBe(390)
    // Dimensions inconnues (page cassée) : forme d'une page portrait 2:3.
    expect(fittedWidth(1280, 800, { width: null, height: null })).toBeCloseTo(533.33, 1)
    expect(fittedWidth(390, 844, { width: null, height: null })).toBe(390)
  })
})

describe('requestWidth', () => {
  const page = { width: 1200, media_type: 'image/jpeg' }
  it('prend la plus petite largeur fixe qui couvre les pixels réels', () => {
    expect(requestWidth(390, 1, page)).toBe(480)
    expect(requestWidth(390, 2, page)).toBe(800)
    expect(requestWidth(320, 1, page)).toBe(320)
  })
  it('sert l’originale plutôt que d’agrandir, et toujours pour une AVIF', () => {
    expect(requestWidth(390, 3, page)).toBeNull() // 1170 → 1280 ≥ 1200
    expect(requestWidth(5000, 2, { width: 9000, media_type: 'image/png' })).toBe(2048)
    expect(requestWidth(100, 1, { width: null, media_type: 'image/png' })).toBe(160)
    expect(requestWidth(100, 1, { width: 1200, media_type: 'image/avif' })).toBeNull()
  })
})

describe('pageUrl', () => {
  const page = { id: 'p', ordinal: 0, width: 1200, height: 1800, media_type: 'image/jpeg', status: 'ok' } as const
  it('demande une AVIF à la taille de l’écran, ou l’originale', () => {
    expect(pageUrl(page, { width: 390, height: 844 }, false, 2)).toBe('/api/pages/p/image?w=800&f=avif')
    expect(pageUrl(page, { width: 390, height: 844 }, true, 2)).toBe('/api/pages/p/image')
  })
})

describe('keyMove', () => {
  it.each([
    ['ArrowLeft', false, 'rtl', 'next'],
    ['ArrowRight', false, 'rtl', 'previous'],
    ['ArrowRight', false, 'ltr', 'next'],
    ['ArrowLeft', false, 'ltr', 'previous'],
    ['PageDown', false, 'rtl', 'next'],
    ['PageUp', false, 'ltr', 'previous'],
    [' ', false, 'rtl', 'next'],
    [' ', true, 'rtl', 'previous'],
    ['Home', false, 'ltr', 'first'],
    ['End', false, 'rtl', 'last'],
    ['a', false, 'ltr', null],
  ] as const)('%s (maj %s) en %s → %s', (key, shift, direction, move) => {
    expect(keyMove(key, shift, direction)).toBe(move)
  })
})

describe('tapAction', () => {
  it('tourne la page sur les côtés, selon le sens, et affiche les commandes au centre', () => {
    expect(tapAction(50, 900, 'rtl')).toBe('next')
    expect(tapAction(850, 900, 'rtl')).toBe('previous')
    expect(tapAction(50, 900, 'ltr')).toBe('previous')
    expect(tapAction(850, 900, 'ltr')).toBe('next')
    expect(tapAction(450, 900, 'rtl')).toBe('toggle')
  })
})

describe('swipeAction', () => {
  const at = (x: number, y = 400) => ({ x, y })
  it('avance vers la droite en manga, vers la gauche en BD', () => {
    expect(swipeAction(at(200), at(320), 900, 'rtl')).toBe('next')
    expect(swipeAction(at(320), at(200), 900, 'rtl')).toBe('previous')
    expect(swipeAction(at(320), at(200), 900, 'ltr')).toBe('next')
  })
  it('ignore les petits gestes, les gestes verticaux et ceux qui partent d’un bord', () => {
    expect(swipeAction(at(200), at(230), 900, 'ltr')).toBeNull()
    expect(swipeAction(at(200, 100), at(280, 400), 900, 'ltr')).toBeNull()
    expect(swipeAction(at(10), at(200), 900, 'rtl')).toBeNull()
    expect(swipeAction(at(890), at(600), 900, 'ltr')).toBeNull()
  })
})

describe('moveIndex', () => {
  it('reste dans le tome', () => {
    expect(moveIndex(0, 'previous', 10)).toBe(0)
    expect(moveIndex(9, 'next', 10)).toBe(9)
    expect(moveIndex(4, 'next', 10)).toBe(5)
    expect(moveIndex(4, 'first', 10)).toBe(0)
    expect(moveIndex(4, 'last', 10)).toBe(9)
    expect(moveIndex(0, 'last', 0)).toBe(0)
  })
})
