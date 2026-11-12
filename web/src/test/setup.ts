import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, beforeEach } from 'vitest'
import i18n from '@/i18n'

// jsdom n'a pas ces API du navigateur, utilisées par le menu déroulant de Radix (Select).
Element.prototype.hasPointerCapture ??= () => false
Element.prototype.releasePointerCapture ??= () => {}
Element.prototype.scrollIntoView ??= () => {}

// Les tests lisent les textes en français, quelle que soit la langue de la machine.
beforeEach(() => i18n.changeLanguage('fr'))

// Sans `globals: true`, Testing Library ne démonte pas tout seul le rendu d'un test.
afterEach(cleanup)
