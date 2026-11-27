// Liens d'invitation (1.4b) : l'administrateur invite un lecteur sur la seule bibliothèque Manga,
// la personne crée son compte par le lien, qui ne sert qu'une fois ; captures aux quatre tailles.
// Sans la session partagée : chaque connexion ici a la sienne.
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

const CAPTURES = fileURLToPath(new URL('./captures/', import.meta.url))
const SIZES = [
  [390, 844],
  [800, 1280],
  [1280, 800],
  [1440, 900],
] as const

test.use({ storageState: { cookies: [], origins: [] } })

test('invitation : lien créé, compte ouvert par le lien, lien consommé', async ({ page, browser }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const reader = `invite${Date.now()}`
  await page.goto('/admin/users')
  await page.getByLabel('Nom du compte').fill('e2e')
  await page.getByLabel('Mot de passe', { exact: true }).fill('e2e mot de passe')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('heading', { name: 'Comptes', level: 1 })).toBeVisible()

  await page.getByRole('button', { name: 'Inviter' }).click()
  const dialog = page.getByRole('dialog', { name: 'Inviter' })
  await dialog.getByRole('checkbox', { name: 'Webtoon' }).uncheck()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}inviter-${width}x${height}.png` })
  }
  await dialog.getByRole('button', { name: 'Créer le lien' }).click()
  const link = (await dialog.locator('code').textContent()) ?? ''
  expect(link).toMatch(/\/invite\/[0-9a-f]{64}$/)
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}inviter-lien-${width}x${height}.png` })
  }
  await dialog.getByRole('button', { name: "C'est noté" }).click()
  const pending = page.getByRole('region', { name: 'Invitations en attente' })
  await expect(pending.getByText('Lecteur · 1 bibliothèque').first()).toBeVisible()
  for (const [width, height] of SIZES) {
    await page.setViewportSize({ width, height })
    await page.screenshot({ path: `${CAPTURES}comptes-invitations-${width}x${height}.png`, fullPage: true })
  }

  // La personne invitée, sur un autre appareil (sans session).
  const other = await browser.newContext({ locale: 'fr-FR' })
  const guest = await other.newPage()
  await guest.goto(link)
  await expect(guest.getByRole('heading', { name: 'Créer votre compte' })).toBeVisible()
  for (const [width, height] of SIZES) {
    await guest.setViewportSize({ width, height })
    await guest.screenshot({ path: `${CAPTURES}invitation-${width}x${height}.png` })
  }
  await guest.getByLabel('Nom du compte').fill(reader)
  await guest.getByLabel('Mot de passe', { exact: true }).fill('invité mot de passe')
  await guest.getByRole('button', { name: 'Créer le compte' }).click()
  const nav = guest.getByRole('navigation', { name: 'Navigation principale' }).first()
  await expect(nav.getByRole('link', { name: `Compte : ${reader}` })).toBeVisible()
  await expect(nav.getByRole('link', { name: 'Manga' })).toBeVisible()
  await expect(nav.getByRole('link', { name: 'Webtoon' })).toHaveCount(0)

  // Le lien a servi : il ne vaut plus rien.
  await other.clearCookies()
  await guest.goto(link)
  await expect(guest.getByRole('heading', { name: "Lien d'invitation non valable" })).toBeVisible()
  for (const [width, height] of SIZES) {
    await guest.setViewportSize({ width, height })
    await guest.screenshot({ path: `${CAPTURES}invitation-invalide-${width}x${height}.png` })
  }
  await other.close()

  // Côté administrateur : le compte apparaît, l'invitation n'est plus en attente.
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.reload()
  await expect(page.getByRole('heading', { name: reader })).toBeVisible()
})
