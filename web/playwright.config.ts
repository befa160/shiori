import { defineConfig, devices } from '@playwright/test'

/** Tests de bout en bout du lecteur : un vrai `shiori serve` (base neuve, bibliothèques
 * générées par `shiori-testkit`) et Chromium. Prérequis : `npm run build` (le binaire de debug
 * sert `web/dist`). Lancement : `npm run e2e`. */
export default defineConfig({
  testDir: 'e2e',
  globalSetup: './e2e/setup.ts',
  // Un seul serveur et une seule base, partagés : les tests ne tournent pas en parallèle.
  workers: 1,
  // Le binaire de debug encode une page AVIF en 2 à 3 s : on vérifie la justesse, pas la vitesse.
  timeout: 180_000,
  expect: { timeout: 30_000 },
  // Pas de nouvel essai : un test instable doit se voir, pas passer au second coup.
  retries: 0,
  reporter: process.env.CI ? [['github'], ['list']] : 'list',
  use: {
    ...devices['Desktop Chrome'],
    baseURL: 'http://127.0.0.1:7879',
    locale: 'fr-FR',
    trace: 'retain-on-failure',
  },
})
