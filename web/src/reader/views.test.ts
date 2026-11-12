import { describe, expect, it } from 'vitest'
import type { PageInfo } from '@/api/types'
import { type View, buildViews, effectiveLayout, viewOfPage, viewParams } from './views'

// `w` : double page (plus large que haute) ; `x` : page cassée (dimensions inconnues).
const pages = (spec: string): PageInfo[] =>
  [...spec].map((c, i) => ({
    id: `p${i}`,
    ordinal: i,
    width: c === 'w' ? 2000 : c === 'x' ? null : 1000,
    height: c === 'x' ? null : 1500,
    media_type: 'image/jpeg',
    status: c === 'x' ? 'broken' : 'ok',
  }))

/** Écriture compacte d'une vue : « 1-2 » pour deux pages, « 3 » pour une, « 4d » / « 4g » pour une moitié. */
const show = (views: View[]) =>
  views
    .map((v) => (v.kind === 'half' ? `${v.page.ordinal}${v.half === 'right' ? 'd' : 'g'}` : v.pages.map((p) => p.ordinal).join('-')))
    .join(' ')

describe('effectiveLayout', () => {
  it('auto : double en paysage, simple en portrait', () => {
    expect(effectiveLayout('auto', { width: 1280, height: 800 })).toBe('double')
    expect(effectiveLayout('auto', { width: 800, height: 1280 })).toBe('single')
    expect(effectiveLayout('single', { width: 1280, height: 800 })).toBe('single')
    expect(effectiveLayout('double', { width: 390, height: 844 })).toBe('double')
  })
})

describe('buildViews en double', () => {
  const double = (spec: string, offset = false) =>
    show(buildViews(pages(spec), { layout: 'double', direction: 'rtl', offset, split: true }))

  it.each([
    ['ppppp', '0 1-2 3-4'],
    ['pppp', '0 1-2 3'],
    ['ppwppp', '0 1 2 3-4 5'], // la double page reste seule, l'appariement reprend après
    ['pppwpp', '0 1-2 3 4-5'],
    ['p', '0'],
    ['', ''],
    ['ppxpp', '0 1-2 3-4'], // une page cassée garde sa place dans la paire
  ])('%s → %s', (spec, expected) => {
    expect(double(spec)).toBe(expected)
  })

  it('le décalage laisse aussi la deuxième page seule', () => {
    expect(double('ppppp', true)).toBe('0 1 2-3 4')
  })
})

describe('buildViews en simple', () => {
  const single = (spec: string, direction: 'rtl' | 'ltr', split = true) =>
    show(buildViews(pages(spec), { layout: 'single', direction, offset: false, split }))

  it('coupe chaque double page en deux moitiés, dans le sens de lecture', () => {
    expect(single('pwp', 'rtl')).toBe('0 1d 1g 2')
    expect(single('pwp', 'ltr')).toBe('0 1g 1d 2')
  })
  it('garde les doubles pages entières si on le demande, et ne coupe jamais une page cassée', () => {
    expect(single('pwp', 'rtl', false)).toBe('0 1 2')
    expect(single('pxp', 'rtl')).toBe('0 1 2')
  })
})

describe('viewOfPage', () => {
  it('trouve la vue qui montre une page, la première moitié pour une page coupée', () => {
    const views = buildViews(pages('pppwp'), { layout: 'double', direction: 'ltr', offset: false, split: true })
    expect(viewOfPage(views, 'p2')).toBe(1)
    const halves = buildViews(pages('pwp'), { layout: 'single', direction: 'ltr', offset: false, split: true })
    expect(viewOfPage(halves, 'p1')).toBe(1)
    // La moitié est dans l'adresse : sans elle, on ne pourrait jamais dépasser une page coupée.
    expect(viewOfPage(halves, 'p1', 'right')).toBe(2)
    expect(viewOfPage(halves, 'p1', 'left')).toBe(1)
    expect(viewParams(halves[2])).toEqual({ page: 'p1', half: 'right' })
    expect(viewParams(halves[0])).toEqual({ page: 'p0' })
    expect(viewOfPage(halves, 'inconnue')).toBe(0)
    expect(viewOfPage(halves, null)).toBe(0)
  })
})
