// Comptes par l'interface (lots 7.7, 7.5, 7.6) : connexion, création d'un lecteur par
// l'administrateur, mot de passe provisoire changé à la première connexion, déconnexion, double
// authentification, jeton d'API. Sans la session partagée des autres tests (elle resterait
// ouverte) : chaque connexion ici a la sienne.
import { createHmac } from 'node:crypto'
import { expect, test } from '@playwright/test'

/** Code TOTP (RFC 6238 : SHA-1, 6 chiffres, 30 s) d'une clé base32, comme une application d'authentification. */
function totp(base32: string): string {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'
  const bits = [...base32.replace(/\s/g, '')].map((c) => alphabet.indexOf(c).toString(2).padStart(5, '0')).join('')
  const key = Buffer.from(bits.match(/.{8}/g)!.map((b) => parseInt(b, 2)))
  const counter = Buffer.alloc(8)
  counter.writeBigUInt64BE(BigInt(Math.floor(Date.now() / 30_000)))
  const hmac = createHmac('sha1', key).update(counter).digest()
  const offset = hmac[19] & 0xf
  return String((hmac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).padStart(6, '0')
}

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

  // Le lecteur active la double authentification…
  await page.goto('/account')
  const totpRegion = page.getByRole('region', { name: 'Double authentification' })
  await totpRegion.getByRole('button', { name: 'Activer' }).click()
  const enable = page.getByRole('dialog', { name: 'Activer la double authentification' })
  await enable.getByLabel('Mot de passe actuel').fill('lecteur mot de passe')
  await enable.getByRole('button', { name: 'Continuer' }).click()
  await expect(enable.getByRole('img', { name: /QR code/ })).toBeVisible()
  const key = (await enable.locator('code').textContent()) ?? ''
  await enable.getByLabel('Code').fill(totp(key))
  await enable.getByRole('button', { name: 'Activer' }).click()
  const codes = page.getByRole('list', { name: 'Codes de secours' }).getByRole('listitem')
  await expect(codes).toHaveCount(10)
  const recovery = (await codes.first().textContent()) ?? ''
  await page.getByRole('button', { name: "C'est noté" }).click()
  await expect(totpRegion.getByText('Active. Il vous reste 10 codes de secours.')).toBeVisible()

  // … puis se reconnecte : mot de passe, puis un code de secours.
  await page.getByRole('main').getByRole('button', { name: 'Se déconnecter' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Se déconnecter' }).click()
  await page.getByLabel('Nom du compte').fill(reader)
  await page.getByLabel('Mot de passe', { exact: true }).fill('lecteur mot de passe')
  await page.getByRole('button', { name: 'Se connecter' }).click()
  await page.getByLabel('Code').fill(recovery)
  await page.getByRole('button', { name: 'Vérifier' }).click()
  await page.goto('/account')
  await expect(page.getByText('Active. Il vous reste 9 codes de secours.')).toBeVisible()

  // Un jeton d'API pour une liseuse : il lit, il ne touche pas au compte.
  const tokens = page.getByRole('region', { name: "Jetons d'API" })
  await tokens.getByRole('button', { name: 'Nouveau jeton' }).click()
  const create = page.getByRole('dialog', { name: 'Nouveau jeton' })
  await create.getByLabel('Nom').fill('Liseuse')
  await create.getByRole('button', { name: 'Créer le jeton' }).click()
  const token = (await page.getByRole('dialog').locator('code').textContent()) ?? ''
  await page.getByRole('button', { name: "C'est noté" }).click()
  await expect(tokens.getByText('Liseuse', { exact: true })).toBeVisible()
  const bearer = { authorization: `Bearer ${token}` }
  expect((await fetch('http://127.0.0.1:7879/api/series', { headers: bearer })).status).toBe(200)
  expect((await fetch('http://127.0.0.1:7879/api/me', { headers: bearer })).status).toBe(403)
})
