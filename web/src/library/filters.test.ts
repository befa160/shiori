import { describe, expect, it } from 'vitest'
import type { SeriesSummary } from '@/api/types'
import { applyFilters, fold, isFiltered, readFilters } from './filters'

const base: SeriesSummary = {
  id: 0,
  library_id: 1,
  name: '',
  volume_count: 1,
  read_count: 0,
  page_count: 30,
  pages_read: 0,
  reading_status: 'unread',
  publication_status: null,
  created_at: '2026-01-01T00:00:00.000Z',
  last_read_at: null,
  cover_page_id: null,
  resume_volume_id: null,
}
// Dans l'ordre du serveur (par nom).
const series: SeriesSummary[] = [
  { ...base, id: 1, name: 'Akira', publication_status: 'completed', created_at: '2026-03-01T00:00:00.000Z' },
  { ...base, id: 2, name: 'Été en pente', reading_status: 'in_progress', last_read_at: '2026-11-01T00:00:00.000Z' },
  { ...base, id: 3, name: 'One Piece', library_id: 2, publication_status: 'ongoing', reading_status: 'read', last_read_at: '2026-11-10T00:00:00.000Z', created_at: '2026-02-01T00:00:00.000Z' },
  { ...base, id: 4, name: 'Piece of Cake', publication_status: 'hiatus' },
]
const ids = (list: SeriesSummary[]) => list.map((s) => s.id)
const filters = (query: string) => readFilters(new URLSearchParams(query))

describe('filtres de séries', () => {
  it('lit l’adresse et ignore les valeurs inconnues', () => {
    expect(filters('')).toEqual({ q: '', read: null, status: null, library: null, sort: 'name' })
    expect(filters('q=one&read=read&status=unknown&library=2&sort=added')).toEqual({
      q: 'one',
      read: 'read',
      status: 'unknown',
      library: 2,
      sort: 'added',
    })
    expect(filters('read=lu&status=fini&library=-1&sort=hasard')).toEqual(filters(''))
    expect(filters('library=1.5')).toMatchObject({ library: null })
  })

  it('cherche sans tenir compte des accents ni de la casse, mot par mot', () => {
    expect(fold('ÉTÉ en Pente')).toBe('ete en pente')
    expect(ids(applyFilters(series, filters('q=ete')))).toEqual([2])
    expect(ids(applyFilters(series, filters('q=PIECE')))).toEqual([3, 4])
    expect(ids(applyFilters(series, filters('q=piece%20one')))).toEqual([3])
    expect(ids(applyFilters(series, filters('q=%20%20')))).toEqual([1, 2, 3, 4])
    expect(ids(applyFilters(series, filters('q=zorro')))).toEqual([])
  })

  it('filtre par lecture, publication (inconnue comprise) et bibliothèque', () => {
    expect(ids(applyFilters(series, filters('read=in_progress')))).toEqual([2])
    expect(ids(applyFilters(series, filters('status=unknown')))).toEqual([2])
    expect(ids(applyFilters(series, filters('status=hiatus')))).toEqual([4])
    expect(ids(applyFilters(series, filters('library=2')))).toEqual([3])
    expect(ids(applyFilters(series, filters('library=1&read=unread')))).toEqual([1, 4])
  })

  it('trie par nom, dernière lecture (jamais lues à la fin) ou ajout', () => {
    expect(ids(applyFilters(series, filters('')))).toEqual([1, 2, 3, 4])
    expect(ids(applyFilters(series, filters('sort=last_read')))).toEqual([3, 2, 1, 4])
    expect(ids(applyFilters(series, filters('sort=added')))).toEqual([1, 3, 2, 4])
  })

  it('sait si un filtre restreint la liste', () => {
    expect(isFiltered(filters('sort=added'))).toBe(false)
    expect(isFiltered(filters('q=%20'))).toBe(false)
    expect(isFiltered(filters('status=ongoing'))).toBe(true)
  })
})
