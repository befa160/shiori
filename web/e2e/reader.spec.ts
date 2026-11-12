// Le lecteur dans un vrai navigateur, contre un vrai serveur : chaque page affichée est la
// bonne image (invariant de lecture), dans tous les modes ; reprise et tome suivant ; captures.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { type APIRequestContext, type Page, expect, test } from '@playwright/test'
import { DATA } from './setup'

type Rgb = [number, number, number]
/** Couleur attendue de chaque page, par nom de fichier, dans l'ordre de lecture. */
const manifest: Record<string, Rgb[]> = JSON.parse(readFileSync(`${DATA}/library/manifest.json`, 'utf8'))
const CAPTURES = fileURLToPath(new URL('./captures/', import.meta.url))
const SIZES = [
  [390, 844],
  [800, 1280],
  [1280, 800],
  [1440, 900],
] as const

interface Volume {
  volume_id: string
  file_name: string
  page_count: number
  series: number
  library: number
}

/** Les tomes de la bibliothèque générée, par nom de fichier. */
async function volumes(request: APIRequestContext): Promise<Record<string, Volume>> {
  const out: Record<string, Volume> = {}
  for (const library of await (await request.get('/api/libraries')).json()) {
    for (const s of await (await request.get(`/api/libraries/${library.id}/series`)).json()) {
      const detail = await (await request.get(`/api/series/${s.id}`)).json()
      for (const v of detail.volumes as Volume[]) out[v.file_name] = { ...v, series: s.id, library: library.id }
    }
  }
  return out
}

/** Les pages à l'écran, une fois chargées : numéro (d'après leur nom accessible) et couleur
 * lue dans l'image elle-même. Les pages générées sont unies : un pixel suffit. */
async function shown(page: Page): Promise<{ n: number; rgb: Rgb }[]> {
  const images = page.locator('main img')
  await expect
    .poll(() => images.evaluateAll((all) => all.length > 0 && all.every((i) => (i as HTMLImageElement).complete && (i as HTMLImageElement).naturalWidth > 0)))
    .toBe(true)
  return images.evaluateAll((all) =>
    Promise.all(all.map(async (element) => {
      const img = element as HTMLImageElement
      // Une image hors de l'écran peut être chargée sans être encore décodée.
      await img.decode()
      const canvas = document.createElement('canvas')
      canvas.width = canvas.height = 1
      const context = canvas.getContext('2d')!
      context.drawImage(img, img.naturalWidth / 4, img.naturalHeight / 4, 1, 1, 0, 0, 1, 1)
      const [r, g, b] = context.getImageData(0, 0, 1, 1).data
      return { n: Number(/\d+/.exec(img.alt)?.[0]), rgb: [r, g, b] as [number, number, number] }
    })),
  )
}

/** Chaque page affichée a la couleur de sa position : ±24 par composante (AVIF avec pertes),
 * alors que deux pages voisines diffèrent d'au moins 139. Rend les numéros vus. */
async function checkShown(page: Page, file: string): Promise<number[]> {
  const pages = await shown(page)
  for (const { n, rgb } of pages) {
    const expected = manifest[file][n - 1]
    const gap = Math.max(...rgb.map((c, i) => Math.abs(c - expected[i])))
    expect(gap, `page ${n} de ${file} : ${rgb} au lieu de ${expected}`).toBeLessThanOrEqual(24)
  }
  return pages.map((p) => p.n)
}

const announced = (page: Page) => page.locator('p.sr-only[aria-live]').textContent()

/** Parcourt un tome paginé jusqu'au bout (flèche dans le sens de lecture) en vérifiant chaque vue. */
async function readThrough(page: Page, file: string, next: 'ArrowLeft' | 'ArrowRight') {
  const seen = new Set<number>()
  for (let views = 0; views < 40; views++) {
    for (const n of await checkShown(page, file)) seen.add(n)
    const before = await announced(page)
    await page.keyboard.press(next)
    // La page change dans une transition (React Router) : on attend l'annonce suivante. Sans
    // changement, c'est la dernière vue (la flèche montre les commandes) ; une flèche perdue en
    // cours de route ferait échouer le contrôle des pages vues ci-dessous.
    const moved = await expect(page.locator('p.sr-only[aria-live]'))
      .not.toHaveText(before ?? '', { timeout: 5_000 })
      .then(() => true, () => false)
    if (!moved) break
  }
  expect([...seen].sort((a, b) => a - b)).toEqual(manifest[file].map((_, i) => i + 1))
}

let all: Awaited<ReturnType<typeof volumes>>
test.beforeAll(async ({ request }) => {
  all = await volumes(request)
})
// Chaque test part d'un tome jamais ouvert.
test.beforeEach(async ({ request }) => {
  for (const v of Object.values(all)) await request.delete(`/api/volumes/${v.volume_id}/progress`)
})

test.describe('invariant de lecture : chaque page affichée est la bonne image', () => {
  test('manga en portrait : pages simples, doubles pages coupées en deux moitiés', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto(`/read/${all['E2E Manga T01.cbz'].volume_id}`)
    await readThrough(page, 'E2E Manga T01.cbz', 'ArrowLeft')
  })

  test('manga en paysage : paires de pages et doubles pages seules', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.goto(`/read/${all['E2E Manga T01.cbz'].volume_id}`)
    await page.locator('main img').first().waitFor()
    await page.keyboard.press('ArrowLeft')
    // Après la couverture seule : deux pages côte à côte (attendues : la page change en transition).
    await expect(page.locator('main img')).toHaveCount(2)
    expect(await checkShown(page, 'E2E Manga T01.cbz')).toHaveLength(2)
    await page.keyboard.press('Home')
    await readThrough(page, 'E2E Manga T01.cbz', 'ArrowLeft')
  })

  test('RAR solid écrit à l’envers de l’ordre de lecture', async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 1280 })
    await page.goto(`/read/${all['E2E Manga T02.cbr'].volume_id}`)
    await readThrough(page, 'E2E Manga T02.cbr', 'ArrowLeft')
  })

  test('webtoon : chaque bande à sa place jusqu’en bas', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto(`/read/${all['E2E Strip T01.cbz'].volume_id}`)
    const seen = new Set<number>()
    for (let screens = 0; screens < 200; screens++) {
      for (const n of await checkShown(page, 'E2E Strip T01.cbz')) seen.add(n)
      const bottom = await page.evaluate(() => {
        window.scrollBy(0, window.innerHeight * 0.9)
        return window.scrollY + window.innerHeight >= document.documentElement.scrollHeight - 1
      })
      if (bottom) break
    }
    for (const n of await checkShown(page, 'E2E Strip T01.cbz')) seen.add(n)
    expect(seen.size).toBe(60)
  })
})

test('reprend à la page lue après un rechargement, puis passe au tome suivant', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto(`/read/${all['E2E Manga T01.cbz'].volume_id}`)
  await page.locator('main img').first().waitFor()
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowLeft')
  // Attendue seulement maintenant : l'enregistrement de la page 1 (1 s après l'ouverture, pendant
  // l'encodage de la première image) ne doit pas passer pour celui de la page atteinte.
  await page.waitForResponse((r) => r.request().method() === 'PUT' && r.url().endsWith('/progress'))
  // Lue après l'enregistrement (1 s plus tard) : l'annonce a fini de suivre les deux flèches.
  const where = await announced(page)
  await page.goto(`/read/${all['E2E Manga T01.cbz'].volume_id}`)
  await page.locator('main img').first().waitFor()
  expect(await announced(page)).toBe(where)
  // Fin du tome : la flèche montre les commandes et le bouton du tome suivant.
  await page.keyboard.press('End')
  await expect(page.locator('p.sr-only[aria-live]')).toHaveText(/12 sur 12$/)
  await page.keyboard.press('ArrowLeft')
  await page.getByRole('link', { name: 'Tome suivant' }).click()
  await expect(page).toHaveURL(new RegExp(all['E2E Manga T02.cbr'].volume_id))
  expect(await checkShown(page, 'E2E Manga T02.cbr')).toEqual([1])
})

test('captures aux quatre tailles', async ({ page }) => {
  const manga = all['E2E Manga T01.cbz']
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    const shot = (name: string) => page.screenshot({ path: `${CAPTURES}${name}-${width}x${height}.png` })
    await page.goto('/')
    await page.getByRole('link', { name: /Manga/ }).first().waitFor()
    await shot('bibliotheques')
    await page.goto(`/libraries/${manga.library}`)
    await page.getByRole('link', { name: /E2E Manga/ }).waitFor()
    await shot('bibliotheque')
    await page.goto(`/series/${manga.series}`)
    await page.getByRole('link', { name: /Tome 1/ }).first().waitFor()
    await shot('serie')
    await page.goto(`/read/${manga.volume_id}`)
    await checkShown(page, 'E2E Manga T01.cbz')
    await page.keyboard.press('Tab')
    await expect(page.getByRole('link', { name: 'Retour' })).toBeFocused()
    await shot('lecteur')
    await page.getByRole('button', { name: 'Affichage' }).click()
    await page.getByRole('radio', { name: 'Simple' }).waitFor()
    await shot('lecteur-affichage')
    await page.goto(`/read/${all['E2E Strip T01.cbz'].volume_id}`)
    await checkShown(page, 'E2E Strip T01.cbz')
    await shot('webtoon')
  }
})
