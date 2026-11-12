import { describe, expect, it } from 'vitest'
import { ApiError } from '@/api/client'
import { createQueryClient, shouldRetry } from './query'
import { cn } from './utils'

describe('shouldRetry', () => {
  it('ne réessaie jamais une réponse 4xx', () => {
    expect(shouldRetry(0, new ApiError(422, 'page_broken', 'cassée'))).toBe(false)
  })
  it('réessaie deux fois une panne serveur ou réseau', () => {
    const down = new ApiError(0, 'network', 'injoignable')
    expect([0, 1, 2].map((n) => shouldRetry(n, down))).toEqual([true, true, false])
    expect(shouldRetry(0, new ApiError(500, 'internal', 'erreur'))).toBe(true)
    expect(shouldRetry(0, new Error('autre'))).toBe(true)
  })
  it('est la règle par défaut du client de requêtes', () => {
    expect(createQueryClient().getDefaultOptions().queries?.retry).toBe(shouldRetry)
  })
})

describe('cn', () => {
  it('garde la dernière classe Tailwind en conflit', () => {
    const hidden = false
    expect(cn('px-2 text-sm', hidden && 'hidden', 'px-4')).toBe('text-sm px-4')
  })
})
