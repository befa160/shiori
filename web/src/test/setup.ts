import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import i18n from '@/i18n'

// jsdom n'a pas ces API du navigateur, utilisées par Radix (menu du Select, curseur du Slider).
Element.prototype.hasPointerCapture ??= () => false
Element.prototype.releasePointerCapture ??= () => {}
Element.prototype.scrollIntoView ??= () => {}
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
}

// Les tests lisent les textes en français, quelle que soit la langue de la machine.
beforeEach(() => i18n.changeLanguage('fr'))

// Sans `globals: true`, Testing Library ne démonte pas tout seul le rendu d'un test.
afterEach(cleanup)
