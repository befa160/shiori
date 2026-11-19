import { defineConfig, type DefaultTheme } from 'vitepress'

const repo = 'https://github.com/befa160/shiori'

/** Barre latérale d'une langue ; `p` : préfixe (`''` pour le français, `/en` pour l'anglais). */
function sidebar(p: string, t: Record<string, string>): DefaultTheme.Sidebar {
  const guide = [
    { text: t.guide, items: [
      { text: t.firstSteps, link: `${p}/guide/` },
      { text: t.libraries, link: `${p}/guide/libraries` },
      { text: t.reading, link: `${p}/guide/reading` },
      { text: t.metadata, link: `${p}/guide/metadata` },
      { text: t.scans, link: `${p}/guide/scans` },
      { text: t.diagnostics, link: `${p}/guide/diagnostics` },
      { text: t.accounts, link: `${p}/guide/accounts` },
      { text: t.settings, link: `${p}/guide/settings` },
    ] },
  ]
  const install = [
    { text: t.installation, items: [
      { text: t.quickStart, link: `${p}/installation/` },
      { text: t.configuration, link: `${p}/installation/configuration` },
      { text: t.network, link: `${p}/installation/network` },
      { text: t.backup, link: `${p}/installation/backup` },
      { text: t.troubleshooting, link: `${p}/installation/troubleshooting` },
    ] },
  ]
  const technical = [
    { text: t.technical, items: [
      { text: t.overview, link: `${p}/technical/` },
      { text: 'Backend', link: `${p}/technical/backend` },
      { text: t.archives, link: `${p}/technical/archives` },
      { text: t.database, link: `${p}/technical/database` },
      { text: 'Frontend', link: `${p}/technical/frontend` },
      { text: t.security, link: `${p}/technical/security` },
      { text: t.quality, link: `${p}/technical/quality` },
    ] },
    { text: t.api, collapsed: false, items: [
      { text: t.conventions, link: `${p}/api/` },
      { text: t.auth, link: `${p}/api/auth` },
      { text: t.account, link: `${p}/api/account` },
      { text: t.users, link: `${p}/api/users` },
      { text: t.libraries, link: `${p}/api/libraries` },
      { text: t.series, link: `${p}/api/series` },
      { text: t.volumes, link: `${p}/api/volumes` },
      { text: t.metadata, link: `${p}/api/metadata` },
      { text: 'Scans', link: `${p}/api/scans` },
      { text: t.health, link: `${p}/api/health` },
      { text: t.settings, link: `${p}/api/settings` },
      { text: t.backup, link: `${p}/api/backups` },
    ] },
    { text: t.development, items: [{ text: t.contributing, link: `${p}/development/` }] },
  ]
  return {
    [`${p}/guide/`]: guide,
    [`${p}/installation/`]: install,
    [`${p}/technical/`]: technical,
    [`${p}/api/`]: technical,
    [`${p}/development/`]: technical,
  }
}

const fr = {
  guide: 'Guide', firstSteps: 'Premiers pas', libraries: 'Bibliothèques', reading: 'Lire', metadata: 'Métadonnées',
  scans: 'Scans, surveillance, santé', diagnostics: 'Codes de diagnostic', accounts: 'Comptes et sécurité', settings: 'Réglages',
  installation: 'Installation', quickStart: 'Démarrage rapide', configuration: 'Configuration', network: 'Réseau et HTTPS',
  backup: 'Sauvegardes', troubleshooting: 'En cas de problème', technical: 'Technique', overview: "Vue d'ensemble",
  archives: 'Archives et scan', database: 'Base de données', security: 'Sécurité', quality: 'Tests, CI, versions',
  api: 'Référence API', conventions: 'Conventions', auth: 'Authentification', account: 'Compte', users: 'Comptes (admin)',
  series: 'Séries', volumes: 'Tomes et pages', health: 'Santé', development: 'Développement', contributing: 'Contribuer',
}
const en = {
  guide: 'Guide', firstSteps: 'First steps', libraries: 'Libraries', reading: 'Reading', metadata: 'Metadata',
  scans: 'Scans, watching, health', diagnostics: 'Diagnostic codes', accounts: 'Accounts and security', settings: 'Settings',
  installation: 'Installation', quickStart: 'Quick start', configuration: 'Configuration', network: 'Network and HTTPS',
  backup: 'Backups', troubleshooting: 'Troubleshooting', technical: 'Technical', overview: 'Overview',
  archives: 'Archives and scan', database: 'Database', security: 'Security', quality: 'Tests, CI, versions',
  api: 'API reference', conventions: 'Conventions', auth: 'Authentication', account: 'Account', users: 'Accounts (admin)',
  series: 'Series', volumes: 'Volumes and pages', health: 'Health', development: 'Development', contributing: 'Contributing',
}

export default defineConfig({
  title: 'Shiori',
  base: '/shiori/',
  cleanUrls: true,
  // Note de conception citée par CLAUDE.md et le code, gardée hors du site.
  srcExclude: ['cbr.md'],
  // http://localhost:7878 : l'adresse de Shiori chez le lecteur, pas une page du site.
  ignoreDeadLinks: 'localhostLinks',
  lastUpdated: true,
  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: '/shiori/favicon.svg' }]],
  markdown: {
    // ```mermaid → composant rendu dans le navigateur (theme/Mermaid.vue).
    config(md) {
      const fence = md.renderer.rules.fence!
      md.renderer.rules.fence = (tokens, idx, options, env, self) => {
        const token = tokens[idx]
        if (token.info.trim() !== 'mermaid') return fence(tokens, idx, options, env, self)
        return `<ClientOnly><Mermaid code="${encodeURIComponent(token.content)}" /></ClientOnly>`
      }
    },
  },
  themeConfig: {
    logo: { light: '/logo-light.svg', dark: '/logo.svg' },
    socialLinks: [{ icon: 'github', link: repo }],
    search: {
      provider: 'local',
      options: {
        locales: {
          root: {
            translations: {
              button: { buttonText: 'Rechercher', buttonAriaLabel: 'Rechercher' },
              modal: {
                noResultsText: 'Aucun résultat pour',
                resetButtonTitle: 'Effacer la recherche',
                footer: { selectText: 'choisir', navigateText: 'naviguer', closeText: 'fermer' },
              },
            },
          },
        },
      },
    },
  },
  locales: {
    root: {
      label: 'Français',
      lang: 'fr',
      description: 'Lecteur manga, BD et webtoon auto-hébergé',
      themeConfig: {
        nav: [
          { text: 'Guide', link: '/guide/' },
          { text: 'Installation', link: '/installation/' },
          { text: 'Technique', link: '/technical/' },
          { text: 'API', link: '/api/' },
        ],
        sidebar: sidebar('', fr),
        editLink: { pattern: `${repo}/edit/main/docs/:path`, text: 'Modifier cette page sur GitHub' },
        docFooter: { prev: 'Page précédente', next: 'Page suivante' },
        outline: { label: 'Sur cette page', level: [2, 3] },
        lastUpdated: { text: 'Mise à jour' },
        returnToTopLabel: 'Retour en haut',
        sidebarMenuLabel: 'Menu',
        darkModeSwitchLabel: 'Apparence',
        lightModeSwitchTitle: 'Thème clair',
        darkModeSwitchTitle: 'Thème sombre',
        langMenuLabel: 'Langue',
        notFound: { title: 'Page introuvable', quote: "Cette page n'existe pas.", linkText: "Retour à l'accueil" },
        footer: { message: 'Publié sous licence MIT.', copyright: 'Shiori' },
      },
    },
    en: {
      label: 'English',
      lang: 'en',
      link: '/en/',
      description: 'Self-hosted manga, comics and webtoon reader',
      themeConfig: {
        nav: [
          { text: 'Guide', link: '/en/guide/' },
          { text: 'Installation', link: '/en/installation/' },
          { text: 'Technical', link: '/en/technical/' },
          { text: 'API', link: '/en/api/' },
        ],
        sidebar: sidebar('/en', en),
        editLink: { pattern: `${repo}/edit/main/docs/:path`, text: 'Edit this page on GitHub' },
        outline: { level: [2, 3] },
        footer: { message: 'Released under the MIT license.', copyright: 'Shiori' },
      },
    },
  },
})
