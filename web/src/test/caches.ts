import { vi } from 'vitest'

/** `caches` en mémoire (jsdom n'a pas l'API Cache) : ce dont se sert `src/offline/`. */
export function fakeCaches() {
  const stores = new Map<string, Map<string, Response>>()
  const key = (request: RequestInfo | URL) => new URL(typeof request === 'string' ? request : 'url' in request ? request.url : request.href, 'http://localhost').href
  const open = async (name: string) => {
    const store = stores.get(name) ?? new Map<string, Response>()
    stores.set(name, store)
    return {
      match: async (request: RequestInfo | URL) => store.get(key(request))?.clone(),
      put: async (request: RequestInfo | URL, response: Response) => void store.set(key(request), response),
      delete: async (request: RequestInfo | URL) => store.delete(key(request)),
      keys: async () => [...store.keys()].map((url) => new Request(url)),
    }
  }
  vi.stubGlobal('caches', { open, delete: async (name: string) => stores.delete(name) })
  // Page contrôlée par le service worker : sans lui, les téléchargements sont refusés.
  Object.defineProperty(navigator, 'serviceWorker', {
    configurable: true,
    // Copié par les tests qui remplacent `navigator` (`{ ...navigator }`).
    enumerable: true,
    value: { controller: {}, addEventListener() {}, removeEventListener() {} },
  })
  /** Adresses gardées dans un cache, chemin et paramètres. */
  return (name: string) => [...(stores.get(name)?.keys() ?? [])].map((url) => url.replace('http://localhost', ''))
}
