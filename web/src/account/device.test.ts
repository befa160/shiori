import { describe, expect, it } from 'vitest'
import { deviceName } from './device'

describe('deviceName', () => {
  it('nomme le navigateur et le système, sans se laisser tromper par les mots empruntés', () => {
    const cases: [string, string][] = [
      ['Mozilla/5.0 (Linux; Android 14; SM-X110) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36', 'Chrome · Android'],
      ['Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/26.0 Chrome/122.0 Mobile Safari/537.36', 'Samsung Internet · Android'],
      ['Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 Edg/131.0', 'Edge · Windows'],
      ['Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0', 'Firefox · Linux'],
      ['Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1', 'Safari · iOS'],
      ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 OPR/115.0', 'Opera · macOS'],
    ]
    for (const [ua, name] of cases) expect(deviceName(ua)).toBe(name)
  })

  it('garde le texte brut d’un client inconnu', () => {
    expect(deviceName('node')).toBe('node')
    expect(deviceName('appareil inconnu')).toBe('appareil inconnu')
  })
})
