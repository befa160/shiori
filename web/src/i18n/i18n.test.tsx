import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError } from '@/api/client'
import { json, mockApi, renderAt } from '@/test/render'
import i18n, { browserLanguage } from '.'
import { en } from './en'
import { errorText } from './errors'
import { fr } from './fr'

afterEach(() => vi.unstubAllGlobals())

describe('browserLanguage', () => {
  it('prend la première langue connue du navigateur, sinon l’anglais', () => {
    expect(browserLanguage(['fr-CA', 'en'])).toBe('fr')
    expect(browserLanguage(['de-DE', 'EN-gb'])).toBe('en')
    expect(browserLanguage(['ja'])).toBe('en')
    expect(browserLanguage([])).toBe('en')
  })
})

describe('errorText', () => {
  const t = i18n.getFixedT('en')
  it('traduit une erreur de l’API d’après son code', () => {
    expect(errorText(new ApiError(409, 'volume_changed', 'le fichier a changé'), t)).toBe(en.errors.volume_changed)
    expect(errorText(new ApiError(0, 'network', 'serveur injoignable'), i18n.getFixedT('fr'))).toBe(fr.errors.network)
    expect(errorText(new ApiError(502, 'http_error', 'erreur HTTP 502'), t)).toBe('HTTP error 502')
  })
  it('précise ce qui est introuvable', () => {
    const error = new ApiError(404, 'not_found', 'série introuvable')
    expect(errorText(error, t, en.series.notFound)).toBe('Series not found')
  })
  it('garde le message du serveur pour un code inconnu ou une autre erreur', () => {
    expect(errorText(new ApiError(409, 'nouveau_code', 'message du serveur'), t)).toBe('message du serveur')
    expect(errorText(new ApiError(404, 'not_found', 'tome introuvable'), t)).toBe('tome introuvable')
    expect(errorText(new Error('JSON invalide'), t)).toBe('JSON invalide')
  })
})

describe('langue de l’interface', () => {
  it('suit le réglage du serveur, et <html lang> avec elle', async () => {
    mockApi({ '/api/settings': () => json({ language: 'en' }), '/api/libraries': () => json([]) })
    renderAt('/')
    expect(await screen.findByRole('heading', { name: 'Libraries' })).toBeInTheDocument()
    expect(document.documentElement.lang).toBe('en')
  })

  it('suit le navigateur tant que personne n’a choisi', async () => {
    vi.spyOn(navigator, 'languages', 'get').mockReturnValue(['en-US'])
    mockApi({ '/api/settings': () => json({ language: null }), '/api/libraries': () => json([]) })
    renderAt('/')
    expect(await screen.findByRole('heading', { name: 'Libraries' })).toBeInTheDocument()
  })

  it('change tout de suite au choix dans le sélecteur et l’enregistre sur le serveur', async () => {
    const fetch = mockApi({
      '/api/libraries': () => json([]),
      '/api/settings': () => json({ language: 'fr' }),
    })
    renderAt('/')
    await screen.findByRole('heading', { name: 'Bibliothèques' })
    await userEvent.click(screen.getByRole('combobox', { name: 'Langue' }))
    await userEvent.click(await screen.findByRole('option', { name: 'English' }))
    expect(await screen.findByRole('heading', { name: 'Libraries' })).toBeInTheDocument()
    const put = fetch.mock.calls.find((c) => c[1]?.method === 'PUT')
    expect(put?.[0]).toBe('/api/settings')
    expect(JSON.parse(String(put?.[1]?.body))).toEqual({ language: 'en' })
  })

  it('revient à la langue d’avant si l’enregistrement échoue', async () => {
    let saves = 0
    mockApi({
      '/api/libraries': () => json([]),
      '/api/settings': () => (saves++ === 0 ? json({ language: 'fr' }) : json({ code: 'internal', message: 'erreur' }, 500)),
    })
    renderAt('/')
    await screen.findByRole('heading', { name: 'Bibliothèques' })
    await userEvent.click(screen.getByRole('combobox', { name: 'Langue' }))
    await userEvent.click(await screen.findByRole('option', { name: 'English' }))
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Bibliothèques' })).toBeInTheDocument())
    expect(i18n.language).toBe('fr')
    // Jamais en silence : une notification dit pourquoi, dans la langue revenue.
    expect(await screen.findByText('Langue non enregistrée, réessayez plus tard')).toBeInTheDocument()
    expect(screen.getByText('Erreur interne du serveur')).toBeInTheDocument()
  })
})
