// Comptes par l'interface (lot 7.7) : connexion, création d'un lecteur par l'administrateur,
// mot de passe provisoire changé à la première connexion, déconnexion. Sans la session partagée
// des autres tests (elle resterait ouverte) : chaque connexion ici a la sienne.
import { expect, test } from '@playwright/test'

test.use({ storageState: { cookies: [], origins: [] } })

test('connexion, création d’un lecteur, première connexion du lecteur, déconnexion', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const reader = `lecteur${Date.now()}`

  // Sans session : la page de connexion, à l'adresse demandée.
  await page.goto('/admin/users')
  await expect(page.getByRole('heading', { name: 'Connexion' })).toBeVisible()
  await page.getByLabel('Nom du compte').fill('e2e')
  await page.getByLabel('Mot de passe', { exact: true }).fill('mauvais')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('alert')).toHaveText('Nom ou mot de passe incorrect')
  await page.getByLabel('Mot de passe', { exact: true }).fill('e2e mot de passe')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await expect(page.getByRole('heading', { name: 'Comptes', level: 1 })).toBeVisible()

  // Un lecteur, sur la seule bibliothèque Manga.
  await page.getByRole('button', { name: 'Nouveau compte' }).click()
  const dialog = page.getByRole('dialog', { name: 'Nouveau compte' })
  await dialog.getByLabel('Nom du compte').fill(reader)
  await dialog.getByRole('checkbox', { name: 'Webtoon' }).uncheck()
  await dialog.getByRole('button', { name: 'Créer le compte' }).click()
  const password = (await dialog.locator('code').textContent()) ?? ''
  expect(password).toHaveLength(16)
  await dialog.getByRole('button', { name: "C'est noté" }).click()
  await expect(page.getByText('Lecteur · 1 bibliothèque · mot de passe provisoire')).toBeVisible()

  // Déconnexion de l'administrateur.
  await page.goto('/account')
  await page.getByRole('main').getByRole('button', { name: 'Se déconnecter' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Se déconnecter' }).click()
  await expect(page.getByRole('heading', { name: 'Connexion' })).toBeVisible()

  // Première connexion du lecteur : son mot de passe d'abord, puis seulement Manga.
  await page.getByLabel('Nom du compte').fill(reader)
  await page.getByLabel('Mot de passe', { exact: true }).fill(password)
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await page.getByLabel('Mot de passe provisoire').fill(password)
  await page.getByLabel('Nouveau mot de passe', { exact: true }).fill('lecteur mot de passe')
  await page.getByLabel('Nouveau mot de passe, encore une fois').fill('lecteur mot de passe')
  await page.getByRole('button', { name: 'Changer le mot de passe' }).click()
  const nav = page.getByRole('navigation', { name: 'Navigation principale' }).first()
  await expect(nav.getByRole('link', { name: 'Manga' })).toBeVisible()
  await expect(nav.getByRole('link', { name: 'Webtoon' })).toHaveCount(0)
  await expect(nav.getByRole('link', { name: 'Administration' })).toHaveCount(0)
  await expect(nav.getByRole('link', { name: `Compte : ${reader}` })).toBeVisible()
})
