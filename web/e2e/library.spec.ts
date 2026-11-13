// Le parcours complet par l'interface, comme un utilisateur : créer une bibliothèque (le scan
// démarre), la voir sur l'accueil et en grille, la retrouver par la recherche, ouvrir une série,
// lire un tome, puis supprimer la bibliothèque. Aucun appel direct à l'API.
import { cpSync, rmSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { DATA } from './setup'

// Une copie du manga généré, propre à cette exécution : un dossier qui ne chevauche aucune
// bibliothèque, même laissée par une exécution précédente interrompue.
const RUN = Date.now()
const ROOT = `${DATA}/parcours-${RUN}`
const NAME = `Parcours ${RUN}`

test.beforeAll(() => cpSync(`${DATA}/library/manga`, ROOT, { recursive: true }))
test.afterAll(() => rmSync(ROOT, { recursive: true, force: true }))

test('de la création d’une bibliothèque à la lecture, puis sa suppression', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const main = page.getByRole('main')

  // Créer : le scan démarre tout seul, et sa fin est annoncée.
  await page.goto('/admin')
  await main.getByRole('button', { name: 'Ajouter une bibliothèque' }).click()
  const dialog = page.getByRole('dialog', { name: 'Nouvelle bibliothèque' })
  await dialog.getByRole('textbox', { name: 'Nom' }).fill(NAME)
  await dialog.getByRole('textbox', { name: 'Dossier' }).fill(ROOT)
  await dialog.getByRole('button', { name: 'Créer et scanner' }).click()
  await expect(page.getByText(`Scan de « ${NAME} » lancé`)).toBeVisible()
  await expect(page.getByText('Scan terminé')).toBeVisible({ timeout: 60_000 })

  // Accueil : l'étagère de la bibliothèque, puis sa grille.
  await page.goto('/')
  const shelf = main.getByRole('region', { name: NAME })
  await expect(shelf.getByRole('link', { name: /^E2E Manga/ })).toBeVisible()
  await shelf.getByRole('link', { name: `Tout voir : ${NAME}` }).click()
  await expect(main.getByRole('heading', { level: 1, name: NAME })).toBeVisible()
  await expect(main.getByRole('link', { name: /^E2E Manga/ })).toHaveAttribute('href', /\/series\/\d+$/)

  // Recherche, par l'onglet du bas sur mobile : sans accents ni majuscules, puis un filtre sans résultat.
  await page.setViewportSize({ width: 390, height: 844 })
  await page.getByRole('navigation', { name: 'Navigation principale' }).getByRole('link', { name: 'Recherche' }).click()
  const search = main.getByRole('searchbox', { name: 'Rechercher une série' })
  await expect(search).toBeFocused()
  await search.fill('e2e MANGA')
  await expect(main.getByText(/^\d+ séries?$/)).toHaveText(/^[12] séries?$/)
  await main.getByRole('combobox', { name: 'Lecture' }).click()
  await page.getByRole('option', { name: 'Lues', exact: true }).click()
  await expect(main.getByText('Aucune série ne correspond.')).toBeVisible()
  await main.getByRole('button', { name: 'Effacer les filtres' }).last().click()
  await expect(search).toHaveValue('')

  // Série, puis lecture du premier tome depuis le bouton principal.
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/admin')
  await page.getByRole('navigation', { name: 'Navigation principale' }).getByRole('link', { name: NAME }).click()
  await main.getByRole('link', { name: /^E2E Manga/ }).first().click()
  await expect(main.getByRole('heading', { level: 1, name: 'E2E Manga' })).toBeVisible()
  await expect(main.getByRole('region', { name: 'Tomes' }).getByRole('listitem')).toHaveCount(2)
  // « Lire », ou « Reprendre » si ce tome a déjà été ouvert (la progression survit à la suppression
  // d'une bibliothèque, c'est voulu).
  await main.getByRole('link', { name: /^(Lire|Reprendre) · Tome 1/ }).click()
  await expect(page).toHaveURL(/\/read\/[0-9a-f]{64}/)
  await expect(page.getByRole('img', { name: /^Page 1 sur 12/ })).toBeVisible()

  // Supprimer, après confirmation : la bibliothèque disparaît de la navigation.
  await page.goto('/admin')
  await main.getByRole('button', { name: `Supprimer ${NAME}` }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Supprimer' }).click()
  await expect(page.getByText(`Bibliothèque « ${NAME} » supprimée`)).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Navigation principale' }).getByRole('link', { name: NAME })).toHaveCount(0)
})
