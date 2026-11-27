// Collections partagées (1.3) : créée depuis le menu d'une série, puce sur la série, page de la
// collection, étagère de l'accueil, renommage et suppression. Sur la bibliothèque « Manga » du setup.
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

const CAPTURES = fileURLToPath(new URL('./captures/', import.meta.url))
const SIZES = [
  [390, 844],
  [800, 1280],
  [1280, 800],
  [1440, 900],
] as const

test('collection : créée avec une série, montrée partout, renommée puis supprimée', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const main = page.getByRole('main')
  await page.goto('/series')
  await main.getByRole('link', { name: /^E2E Manga/ }).first().click()
  await expect(main.getByRole('heading', { level: 1, name: 'E2E Manga' })).toBeVisible()

  await main.getByRole('button', { name: 'Actions pour E2E Manga' }).click()
  await page.getByRole('menuitem', { name: 'Collections…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Collections de « E2E Manga »' })
  await dialog.getByLabel('Nouvelle collection, avec cette série').fill('E2E Shōnen')
  await dialog.getByRole('button', { name: 'Créer' }).click()
  await expect(dialog.getByRole('checkbox', { name: 'E2E Shōnen' })).toBeChecked()
  await page.keyboard.press('Escape')
  const chip = main.getByRole('list', { name: 'Collections' }).getByRole('link', { name: 'E2E Shōnen' })
  await expect(chip).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}serie-collections-${width}x${height}.png` })
  }

  await chip.click()
  await expect(main.getByRole('heading', { level: 1, name: 'E2E Shōnen' })).toBeVisible()
  await expect(main.getByRole('link', { name: /^E2E Manga/ })).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}collection-${width}x${height}.png` })
  }
  await page.goto('/')
  const shelf = main.getByRole('region', { name: 'Collections' })
  await expect(shelf.getByRole('link', { name: /E2E Shōnen/ })).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await shelf.scrollIntoViewIfNeeded()
    await page.screenshot({ path: `${CAPTURES}accueil-collections-${width}x${height}.png` })
  }

  await page.setViewportSize({ width: 1280, height: 800 })
  await page.getByRole('navigation').getByRole('link', { name: 'Collections' }).click()
  await main.getByRole('link', { name: /E2E Shōnen/ }).click()
  await main.getByRole('button', { name: 'Renommer' }).click()
  const rename = page.getByRole('dialog', { name: 'Renommer « E2E Shōnen »' })
  await rename.getByLabel('Nom').fill('E2E Nekketsu')
  await rename.getByRole('button', { name: 'Enregistrer' }).click()
  await expect(main.getByRole('heading', { level: 1, name: 'E2E Nekketsu' })).toBeVisible()
  await main.getByRole('button', { name: 'Supprimer' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Supprimer' }).click()
  await expect(page).toHaveURL(/\/collections$/)
  await expect(main.getByRole('link', { name: /E2E Nekketsu/ })).toHaveCount(0)
})
