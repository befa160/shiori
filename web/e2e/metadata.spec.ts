// Métadonnées (phase 5), de bout en bout : un tome avec ComicInfo.xml donne titre, autrice, résumé
// et genre ; un champ modifié à la main est verrouillé, résiste à un rescan et au renommage du
// dossier (même série, lot 5.1), puis se rend aux fichiers. Aucun appel réseau (sources web
// désactivées par défaut). La bibliothèque est préparée par l'API ; le reste par l'interface.
import { cpSync, renameSync, rmSync } from 'node:fs'
import { type APIRequestContext, expect, test } from '@playwright/test'
import { DATA } from './setup'

const RUN = Date.now()
const ROOT = `${DATA}/meta-${RUN}`

test.beforeAll(() => cpSync(`${DATA}/library/meta`, ROOT, { recursive: true }))
test.afterAll(() => rmSync(ROOT, { recursive: true, force: true }))

/** Passe complète de la bibliothèque, attendue jusqu'au bout. */
async function scan(request: APIRequestContext, library: number) {
  expect((await request.post('/api/admin/scans', { data: { library_id: library } })).ok()).toBe(true)
  await expect.poll(async () => (await request.get('/api/admin/scans/current')).status(), { timeout: 60_000 }).toBe(204)
}

test('ComicInfo affiché, champ modifié gardé au rescan et au renommage, puis rendu aux fichiers', async ({ page, request }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  const created = await request.post('/api/libraries', { data: { name: `Méta ${RUN}`, root: ROOT, kind: 'manga' } })
  const library: number = (await created.json()).id
  await scan(request, library)
  const main = page.getByRole('main')

  // Le titre du fichier l'emporte sur le nom du dossier (Q34) ; autrice, résumé et genre suivent.
  await page.goto(`/libraries/${library}`)
  await main.getByRole('link', { name: /^Série Méta E2E/ }).click()
  await expect(main.getByRole('heading', { level: 1, name: 'Série Méta E2E' })).toBeVisible()
  const url = page.url()
  await expect(main.getByText('Autrice E2E', { exact: true })).toBeVisible()
  await expect(main.getByText('Résumé venu du fichier.')).toBeVisible()
  await expect(main.getByRole('list', { name: 'Genres et tags' })).toHaveText('Aventure')

  // Modifier le résumé : verrouillé, et affiché.
  await main.getByRole('button', { name: 'Modifier' }).click()
  let dialog = page.getByRole('dialog', { name: 'Modifier « Série Méta E2E »' })
  await dialog.getByLabel('Résumé', { exact: true }).fill('Mon résumé, écrit à la main.')
  await dialog.getByRole('button', { name: 'Enregistrer' }).click()
  await expect(page.getByText('Modifications enregistrées')).toBeVisible()
  await expect(main.getByText('Mon résumé, écrit à la main.')).toBeVisible()

  // Dossier renommé sur le disque, puis rescan : même série (même adresse), modification gardée.
  renameSync(`${ROOT}/E2E Meta`, `${ROOT}/E2E Meta (renommé)`)
  await scan(request, library)
  await page.goto(url)
  await expect(main.getByRole('heading', { level: 1, name: 'Série Méta E2E' })).toBeVisible()
  await expect(main.getByText('Mon résumé, écrit à la main.')).toBeVisible()

  // Rendu aux fichiers : le résumé du ComicInfo revient.
  await main.getByRole('button', { name: 'Modifier' }).click()
  dialog = page.getByRole('dialog', { name: 'Modifier « Série Méta E2E »' })
  await expect(dialog.getByText('Modifié à la main')).toBeVisible()
  await dialog.getByRole('button', { name: 'Rendre « Résumé » aux fichiers' }).click()
  await expect(dialog.getByLabel('Résumé', { exact: true })).toHaveValue('Résumé venu du fichier.')
  await page.keyboard.press('Escape')
  // Fenêtre fermée (focus rendu à « Modifier ») avant d'ouvrir un menu, sinon ce focus le referme.
  await expect(dialog).toBeHidden()
  await expect(main.getByText('Résumé venu du fichier.')).toBeVisible()

  // Tome (lot 1.2b) : titre modifié depuis son menu, couverture choisie dans le lecteur.
  await main.getByRole('button', { name: 'Actions pour Tome 1' }).click()
  await page.getByRole('menuitem', { name: 'Modifier' }).click()
  dialog = page.getByRole('dialog', { name: /^Modifier « / })
  await dialog.getByRole('textbox', { name: 'Titre' }).fill('Titre du tome E2E')
  await dialog.getByRole('button', { name: 'Enregistrer' }).click()
  await expect(dialog).toBeHidden()
  const card = main.getByRole('link', { name: /Titre du tome E2E/ })
  await expect(card).toBeVisible()
  await card.click()
  await expect(page).toHaveURL(/\/read\//)
  // Tome d'une seule page : la couverture choisie est la même image, l'appel au serveur est vérifié.
  const volume = new URL(page.url()).pathname.split('/').pop()
  const [{ id: chosen }] = await (await request.get(`/api/volumes/${volume}/pages`)).json()
  await page.getByRole('img', { name: /^Page 1 sur 1/ }).waitFor()
  await page.keyboard.press('Tab')
  await page.getByRole('button', { name: 'Affichage' }).click()
  await page.getByRole('button', { name: 'Utiliser comme couverture du tome' }).click()
  await expect(page.getByText('Couverture du tome changée')).toBeVisible()
  const detail = await (await request.get(`/api/volumes/${volume}`)).json()
  expect([detail.metadata.cover_page_id, detail.locked]).toEqual([chosen, ['cover_page_id', 'title']])
  await page.goto(url)
  await expect(main.getByRole('link', { name: /Titre du tome E2E/ }).locator('img')).toHaveAttribute('src', new RegExp(`/api/pages/${chosen}/`))

  expect((await request.delete(`/api/libraries/${library}`)).ok()).toBe(true)
})
