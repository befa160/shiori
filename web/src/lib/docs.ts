/** Adresse de la documentation publiée, dans la langue de l'interface (français à la racine). */
const DOCS = 'https://befa160.github.io/shiori/'

export function docsUrl(language: string, page = ''): string {
  return `${DOCS}${language.startsWith('en') ? 'en/' : ''}${page}`
}
