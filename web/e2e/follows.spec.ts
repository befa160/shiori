// Nouveautés (1.3) : suivre une série, y déposer un tome, le voir en tête de l'accueil, puis
// « Tout marquer comme vu ». La bibliothèque est préparée par l'API ; le reste par l'interface.
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
test.afterAll(() => rmSync(ROOT, { recursive: true, force: true }))

/** Passe complète de la bibliothèque, attendue jusqu'au bout. */
async function scan(request: APIRequestContext, library: number) {
  expect((await request.post('/api/admin/scans', { data: { library_id: library } })).ok()).toBe(true)
  await expect.poll(async () => (await request.get('/api/admin/scans/current')).status(), { timeout: 60_000 }).toBe(204)
}

test('suivre une série : son nouveau tome en tête de l’accueil, jusqu’à « Tout marquer comme vu »', async ({ page, request }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const created = await request.post('/api/libraries', { data: { name: `Suivie ${RUN}`, root: ROOT, kind: 'manga' } })
  const library: number = (await created.json()).id
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
})
