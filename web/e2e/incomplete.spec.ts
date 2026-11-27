// Séries incomplètes (1.4e) : nombre de tomes saisi dans « Modifier » sur « E2E Manga » (T01 et
// T02 présents), tomes présents sur le total et manquants sur la page de la série, filtre
// « Incomplète » de la liste ; captures aux quatre tailles. Les champs sont rendus à la fin.
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

const CAPTURES = fileURLToPath(new URL('./captures/', import.meta.url))
const SIZES = [
  [390, 844],
  [800, 1280],
  [1280, 800],
  [1440, 900],
] as const

let seriesUrl = ''
test.afterAll(async ({ request }) => {
  const id = seriesUrl.split('/').pop()
  if (id) await request.patch(`/api/series/${id}`, { data: { unlock: ['volume_count', 'publication_status'] } })
})

test('série incomplète : 2 / 4 tomes, manquants 3–4, filtre « Incomplète »', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const main = page.getByRole('main')
  await page.goto('/series')
  await main.getByRole('link', { name: /^E2E Manga/ }).first().click()
  await expect(main.getByRole('heading', { level: 1, name: 'E2E Manga' })).toBeVisible()
  seriesUrl = page.url()

  await main.getByRole('button', { name: 'Modifier' }).click()
  const dialog = page.getByRole('dialog', { name: /Modifier/ })
  await dialog.getByLabel('Nombre de tomes').fill('4')
  await dialog.getByLabel('Statut de publication').click()
  await page.getByRole('option', { name: 'Publication terminée' }).click()
  await dialog.getByRole('button', { name: 'Enregistrer' }).click()
  await expect(dialog).toBeHidden()
  await expect(main.getByText(/Publication terminée · 2 \/ 4 tomes/)).toBeVisible()
  await expect(main.getByText('Tomes manquants : 3–4')).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}serie-incomplete-${width}x${height}.png` })
  }

  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/series?status=incomplete')
  await expect(main.getByRole('link', { name: /^E2E Manga/ })).toBeVisible()
  await expect(main.getByRole('link', { name: /^E2E PDF/ })).toHaveCount(0)
})
