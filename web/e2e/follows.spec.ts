// Listes personnelles (1.3) : suivre une série, y déposer un tome, le voir à l'accueil, puis
// « Tout marquer comme vu » ; puis « À lire », quitté en commençant. La bibliothèque est préparée
// par l'API ; le reste par l'interface.
import { cpSync, rmSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { type APIRequestContext, expect, test } from '@playwright/test'
import { DATA } from './setup'

const CAPTURES = fileURLToPath(new URL('./captures/', import.meta.url))
const SIZES = [
  [390, 844],
  [800, 1280],
  [1280, 800],
  [1440, 900],
] as const
const RUN = Date.now()
const ROOT = `${DATA}/follow-${RUN}`

test.beforeAll(() => cpSync(`${DATA}/library/follow`, ROOT, { recursive: true }))
let library = 0
// Bibliothèque retirée avant ses fichiers : surveillée, elle lancerait sinon un scan qui gênerait
// le parcours suivant (un seul scan à la fois).
test.afterAll(async ({ request }) => {
  if (library) await request.delete(`/api/libraries/${library}`)
  rmSync(ROOT, { recursive: true, force: true })
})

/** Passe complète de la bibliothèque, attendue jusqu'au bout. */
async function scan(request: APIRequestContext, library: number) {
  expect((await request.post('/api/admin/scans', { data: { library_id: library } })).ok()).toBe(true)
  await expect.poll(async () => (await request.get('/api/admin/scans/current')).status(), { timeout: 60_000 }).toBe(204)
}

test('suivre une série : son nouveau tome à l’accueil, jusqu’à « Tout marquer comme vu »', async ({ page, request }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const created = await request.post('/api/libraries', { data: { name: `Suivie ${RUN}`, root: ROOT, kind: 'manga' } })
  library = (await created.json()).id
  await scan(request, library)
  const main = page.getByRole('main')

  await page.goto(`/libraries/${library}`)
  await main.getByRole('link', { name: /^E2E Suivie/ }).click()
  const follow = main.getByRole('button', { name: 'Suivre' })
  await expect(follow).toHaveAttribute('aria-pressed', 'false')
  await follow.click()
  await expect(follow).toHaveAttribute('aria-pressed', 'true')
  // Le tome déjà là n'est pas nouveau.
  await page.goto('/')
  await expect(main.getByRole('heading', { level: 1, name: 'Accueil' })).toBeVisible()
  await expect(main.getByRole('region', { name: 'Nouveautés' })).toHaveCount(0)

  cpSync(`${DATA}/library/follow-later/E2E Suivie T02.cbz`, `${ROOT}/E2E Suivie/E2E Suivie T02.cbz`)
  await scan(request, library)
  await page.goto('/')
  const shelf = main.getByRole('region', { name: 'Nouveautés' })
  const card = shelf.getByRole('link', { name: /E2E Suivie/ })
  await expect(card).toContainText('Tome 2')
  await expect(card).toContainText('Nouveau')
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await card.scrollIntoViewIfNeeded()
    await page.screenshot({ path: `${CAPTURES}accueil-nouveautes-${width}x${height}.png` })
  }
  await page.setViewportSize({ width: 1280, height: 800 })
  await shelf.getByRole('button', { name: 'Tout marquer comme vu' }).click()
  await expect(shelf).toHaveCount(0)
  await page.reload()
  await expect(main.getByRole('heading', { level: 1, name: 'Accueil' })).toBeVisible()
  await expect(main.getByRole('region', { name: 'Nouveautés' })).toHaveCount(0)

  // « À lire » (1.3c) : depuis la page de la série, puis l'étagère de l'accueil.
  await page.goto(`/libraries/${library}`)
  await main.getByRole('link', { name: /^E2E Suivie/ }).click()
  const toRead = main.getByRole('button', { name: 'À lire' })
  await toRead.click()
  await expect(toRead).toHaveAttribute('aria-pressed', 'true')
  await page.goto('/')
  const waiting = main.getByRole('region', { name: 'À lire' })
  await expect(waiting.getByRole('link', { name: /^E2E Suivie/ })).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await waiting.scrollIntoViewIfNeeded()
    await page.screenshot({ path: `${CAPTURES}accueil-a-lire-${width}x${height}.png` })
  }
  // Commencée : elle quitte « À lire ».
  await waiting.getByRole('link', { name: /^E2E Suivie/ }).click()
  const saved = page.waitForResponse((r) => r.request().method() === 'PUT' && r.url().endsWith('/progress'))
  await main.getByRole('link', { name: /Lire · Tome 1/ }).click()
  await saved
  await page.goto('/')
  await expect(main.getByRole('heading', { level: 1, name: 'Accueil' })).toBeVisible()
  await expect(main.getByRole('region', { name: 'À lire' })).toHaveCount(0)
})
