// PWA (lot 6.5a) : le service worker garde les pages vues et la dernière interface ; hors ligne,
// l'application s'ouvre encore et le dit.
import { expect, test } from '@playwright/test'

test('le service worker garde les pages lues et ouvre l’interface hors ligne', async ({ page, context, request }) => {
  const manifest = await (await request.get('/manifest.webmanifest')).json()
  expect(manifest).toMatchObject({ id: '/', display: 'standalone', start_url: '/' })

  // Premier chargement : le service worker s'installe ; il ne contrôle la page qu'au suivant.
  await page.goto('/')
  await page.evaluate(() => navigator.serviceWorker.ready)
  await page.reload()
  expect(await page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)

  const [library] = await (await request.get('/api/libraries')).json()
  const [series] = await (await request.get(`/api/libraries/${library.id}/series`)).json()
  const [volume] = (await (await request.get(`/api/series/${series.id}`)).json()).volumes
  await page.goto(`/read/${volume.volume_id}`)
  await expect(page.getByRole('img', { name: /^Page 1 sur / })).toBeVisible()
  // La page affichée et les suivantes préchargées sont gardées.
  await expect
    .poll(() => page.evaluate(async () => (await (await caches.open('shiori-images-v1')).keys()).map((r) => new URL(r.url).pathname)))
    .toContainEqual(expect.stringMatching(/^\/api\/pages\/[0-9a-f]+\/image$/))

  await context.setOffline(true)
  await page.goto('/')
  await expect(page.getByRole('status').filter({ hasText: 'Hors ligne' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: 'Navigation principale' }).first()).toBeVisible()
  await context.setOffline(false)
})

test('un tome téléchargé se lit sans réseau, et la page lue part au retour du réseau', async ({ page, context, request }) => {
  const [library] = await (await request.get('/api/libraries')).json()
  const [series] = await (await request.get(`/api/libraries/${library.id}/series`)).json()
  const detail = await (await request.get(`/api/series/${series.id}`)).json()
  const volume = detail.volumes[0]
  await request.delete(`/api/volumes/${volume.volume_id}/progress`)

  await page.goto('/')
  await page.evaluate(() => navigator.serviceWorker.ready)
  await page.goto(`/series/${series.id}`)
  const main = page.getByRole('main')
  await main.getByRole('button', { name: /^Actions pour / }).first().click()
  await page.getByRole('menuitem', { name: 'Télécharger pour lire hors ligne' }).click()
  await expect(main.getByText('Téléchargé')).toBeVisible()

  await context.setOffline(true)
  await page.goto('/downloads')
  await page.getByRole('link', { name: new RegExp(`^Lire ${detail.name}`) }).first().click()
  // Chaque page affichée vient du cache, jamais d'erreur ni d'image vide.
  const images = page.getByRole('main').getByRole('img', { name: /^Page \d+ sur / })
  for (let step = 0; step < 3; step++) {
    await expect(images.first()).toBeVisible()
    await expect
      .poll(() => images.evaluateAll((all) => all.every((i) => (i as HTMLImageElement).complete && (i as HTMLImageElement).naturalWidth > 0)))
      .toBe(true)
    await page.keyboard.press('PageDown')
  }
  // La page lue attend le réseau sur l'appareil…
  await expect.poll(() => page.evaluate(() => localStorage.getItem('shiori.pendingProgress'))).toContain(volume.volume_id)
  // … et part au retour du réseau.
  await context.setOffline(false)
  await page.evaluate(() => dispatchEvent(new Event('online')))
  await expect.poll(async () => (await (await request.get(`/api/volumes/${volume.volume_id}`)).json()).progress?.ordinal).toBeGreaterThan(0)
})
