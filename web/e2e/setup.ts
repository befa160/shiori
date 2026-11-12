// Prépare une instance de Shiori pour les tests : bibliothèques générées (couleur unique par
// page, voir `backend/crates/shiori-testkit/examples/e2e_library.rs`), base neuve, scan fini.
import { type ChildProcess, execFileSync, spawn } from 'node:child_process'
import { mkdirSync, rmSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const backend = fileURLToPath(new URL('../../backend', import.meta.url))
export const DATA = fileURLToPath(new URL('./.data', import.meta.url))
const BASE = 'http://127.0.0.1:7879'

async function api(path: string, init?: RequestInit) {
  const response = await fetch(`${BASE}/api${path}`, {
    ...init,
    headers: init?.body ? { 'content-type': 'application/json' } : undefined,
  })
  if (!response.ok) throw new Error(`${init?.method ?? 'GET'} ${path} : HTTP ${response.status}`)
  return response.status === 204 ? null : response.json()
}

async function until(check: () => Promise<boolean>, what: string, seconds = 60) {
  for (let i = 0; i < seconds * 4; i++) {
    if (await check().catch(() => false)) return
    await new Promise((done) => setTimeout(done, 250))
  }
  throw new Error(`délai dépassé : ${what}`)
}

export default async function setup() {
  rmSync(DATA, { recursive: true, force: true })
  mkdirSync(DATA, { recursive: true })
  const cargo = (...args: string[]) => execFileSync('cargo', args, { cwd: backend, stdio: 'inherit' })
  cargo('run', '-q', '-p', 'shiori-testkit', '--example', 'e2e_library', '--', `${DATA}/library`)
  cargo('build', '-q', '-p', 'shiori')
  const exe = `${backend}/target/debug/shiori${process.platform === 'win32' ? '.exe' : ''}`
  const server: ChildProcess = spawn(
    exe,
    ['serve', '--listen', '127.0.0.1:7879', '--db', `${DATA}/shiori.db`, '--cache-dir', `${DATA}/cache`],
    { stdio: 'inherit' },
  )
  // Un serveur qui s'arrête (port déjà pris…) fait échouer tout de suite, au lieu de laisser la
  // vérification de démarrage répondre grâce à un autre serveur resté sur le port.
  const died = new Promise<never>((_, reject) =>
    server.on('exit', (code) => reject(new Error(`shiori serve s’est arrêté (code ${code})`))),
  )
  died.catch(() => {}) // l'arrêt voulu, à la fin des tests, n'est pas une erreur
  try {
    await Promise.race([until(async () => (await fetch(`${BASE}/api/health`)).ok, 'démarrage du serveur'), died])
    await Promise.race([createLibraries(), died])
  } catch (error) {
    // Jamais de serveur orphelin sur le port après un échec.
    server.kill()
    throw error
  }
  return () => {
    server.kill()
  }
}

async function createLibraries() {
  for (const [name, kind] of [
    ['Manga', 'manga'],
    ['Webtoon', 'webtoon'],
  ]) {
    const library = await api('/libraries', {
      method: 'POST',
      body: JSON.stringify({ name, root: `${DATA}/library/${kind}`, kind }),
    })
    await api('/admin/scans', { method: 'POST', body: JSON.stringify({ library_id: library.id }) })
    await until(async () => (await api('/admin/scans/current')) === null, `scan de ${name}`)
  }
}
