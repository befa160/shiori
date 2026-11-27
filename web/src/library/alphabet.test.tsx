import { act, fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Library, SeriesSummary } from '@/api/types'
import { json, mockApi, renderAt } from '@/test/render'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  // Absente de jsdom, posée par un test.
  delete (Element.prototype as Partial<Element>).scrollIntoView
})

const library: Library = { id: 1, name: 'Mangas', root: '/m', kind: 'manga', watch: true, series_count: 40, volume_count: 40 }
const base = {
  library_id: 1, volume_count: 1, read_count: 0, page_count: 30, pages_read: 0, reading_status: 'unread', publication_status: null,
  created_at: '2026-01-01T00:00:00.000Z', last_read_at: null, to_read_at: null, cover_page_id: null, resume_volume_id: null,
  missing_volumes: null, cover_image_id: null, alt_titles: [] as string[], authors: [] as string[], tags: [] as string[],
} as const
// 40 séries : 20 en A, 20 en C ; rien en B ni en #.
const many: SeriesSummary[] = Array.from({ length: 40 }, (_, i) => ({ ...base, id: i + 1, name: `${i < 20 ? 'Akira' : 'Cobra'} ${i}` }))
const serve = (list = many) =>
  mockApi({ '/api/libraries': () => json([library]), '/api/series': () => json(list), '/api/libraries/1/series': () => json(list) })

describe('index alphabétique', () => {
  it('liste les lettres, grise celles sans série et va à la première série de la lettre', async () => {
    serve()
    const scroll = vi.fn()
    Element.prototype.scrollIntoView = scroll
    renderAt('/series')
    const index = await screen.findByRole('navigation', { name: 'Index alphabétique' })
    expect(within(index).getAllByRole('button')).toHaveLength(27)
    expect(within(index).getByRole('button', { name: 'Aller à la lettre B' })).toBeDisabled()
    await userEvent.click(within(index).getByRole('button', { name: 'Aller à la lettre C' }))
    expect(scroll).toHaveBeenCalledWith({ block: 'start' })
    expect(scroll.mock.contexts[0]).toHaveAttribute('data-letter', 'C')
    expect((scroll.mock.contexts[0] as HTMLElement).textContent).toContain('Cobra 20')
  })

  it('pas d’index pour une petite liste ni hors du tri par titre', async () => {
    serve(many.slice(0, 29))
    const { unmount } = renderAt('/series')
    expect(await screen.findByText('29 séries')).toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: 'Index alphabétique' })).toBeNull()
    unmount()
    serve()
    renderAt('/series?sort=added')
    expect(await screen.findByText('40 séries')).toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: 'Index alphabétique' })).toBeNull()
  })

  it('poignée : paraît au défilement, montre la lettre pendant le glissement, puis se cache', async () => {
    serve()
    renderAt('/libraries/1')
    await screen.findByRole('navigation', { name: 'Index alphabétique' })
    const track = screen.getByTestId('scroll-handle')
    const thumb = track.firstElementChild as HTMLElement
    // Piste de 400 px qui commence à 100 px du haut.
    track.getBoundingClientRect = () => ({ top: 100, height: 400 }) as DOMRect
    expect(thumb).toHaveClass('opacity-0')

    vi.useFakeTimers()
    // Page de 3 000 px dans une fenêtre de 768 px (jsdom) : défilée de moitié.
    Object.defineProperty(document.documentElement, 'scrollHeight', { configurable: true, value: 3000 })
    vi.stubGlobal('scrollY', 1116)
    fireEvent.scroll(window)
    expect(thumb).toHaveClass('opacity-100')
    expect(thumb.style.top).toBe('calc(50% - 1.5rem)')
    act(() => vi.advanceTimersByTime(1500))
    expect(thumb).toHaveClass('opacity-0')

    // Glisser : capture du pointeur (absente de jsdom), défilement et lettre de l'écran.
    const scrollTo = vi.fn()
    vi.stubGlobal('scrollTo', scrollTo)
    let captured = false
    thumb.setPointerCapture = () => (captured = true)
    thumb.hasPointerCapture = () => captured
    fireEvent.pointerDown(thumb, { clientY: 0 })
    expect(scrollTo).toHaveBeenLastCalledWith({ top: 0 })
    // jsdom : toutes les cartes en haut de l'écran, la dernière lettre l'emporte.
    expect(within(thumb).getByText('C')).toBeInTheDocument()
    fireEvent.pointerMove(thumb, { clientY: 300 })
    expect(scrollTo).toHaveBeenLastCalledWith({ top: 1116 })
    fireEvent.pointerMove(thumb, { clientY: 900 })
    expect(scrollTo).toHaveBeenLastCalledWith({ top: 2232 })
    fireEvent.pointerUp(thumb)
    expect(within(thumb).queryByText('C')).toBeNull()
    captured = false
    fireEvent.pointerMove(thumb, { clientY: 300 })
    expect(scrollTo).toHaveBeenCalledTimes(3)
    fireEvent.pointerDown(thumb, { clientY: 0 })
    fireEvent.pointerCancel(thumb)
    expect(within(thumb).queryByText('C')).toBeNull()
  })
})
