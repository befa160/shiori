// Catalogue source : chaque texte de l'interface. `en.ts` doit avoir exactement les mêmes clés
// (vérifié par le compilateur). Pluriels : suffixes `_one` / `_other` (règles d'Intl.PluralRules).
export const fr = {
  common: {
    loading: 'Chargement…',
    retry: 'Réessayer',
  },
  layout: {
    home: 'Shiori, accueil',
    language: 'Langue',
    languageNotSaved: 'Langue non enregistrée, réessayez plus tard',
  },
  notFound: {
    title: 'Page introuvable',
    home: "Retour à l'accueil",
  },
  libraries: {
    title: 'Bibliothèques',
    empty: "Aucune bibliothèque pour l'instant.",
    emptyHelp: "En attendant l'écran d'administration, créez-en une par l'API ({{route}}) puis lancez un scan.",
  },
  library: {
    fallbackName: 'Bibliothèque',
    back: 'Bibliothèques',
    empty: "Aucune série : la bibliothèque n'a pas encore été scannée, ou son dossier est vide.",
    notFound: 'Bibliothèque introuvable',
  },
  series: {
    fallbackName: 'Série',
    back: 'Séries',
    empty: 'Aucun tome dans cette série.',
    notFound: 'Série introuvable',
  },
  volume: {
    volume_one: 'Tome {{range}}',
    volume_other: 'Tomes {{range}}',
    chapter_one: 'Chapitre {{range}}',
    chapter_other: 'Chapitres {{range}}',
    special: 'Spécial',
    specialNumber: 'Spécial {{range}}',
    unknown: 'N° {{range}}',
    read: 'Lu',
    progress: 'Progression de {{label}}',
    statusError: 'Erreur',
    statusWarning: 'Avertissement',
  },
  reader: {
    back: 'Retour',
    next: 'Page suivante',
    previous: 'Page précédente',
    page: 'Page',
    pageOf: 'Page {{current}} sur {{total}}',
    controls: 'Commandes du lecteur',
    empty: "Ce tome n'a aucune page lisible.",
    notFound: 'Tome introuvable',
    pageFailed: "Cette page n'a pas pu être affichée",
  },
  kinds: {
    manga: 'Manga',
    comics: 'BD / comics',
    webtoon: 'Webtoon',
  },
  counts: {
    series_one: '{{count, number}} série',
    series_other: '{{count, number}} séries',
    volumes_one: '{{count, number}} tome',
    volumes_other: '{{count, number}} tomes',
    pages_one: '{{count, number}} page',
    pages_other: '{{count, number}} pages',
  },
  // Erreurs de l'API, par code stable (le message du serveur, en français, sert de secours).
  errors: {
    network: 'Serveur injoignable',
    http_error: 'Erreur HTTP {{status}}',
    internal: 'Erreur interne du serveur',
    invalid_id: 'Identifiant invalide',
    page_broken: "Page illisible dans l'archive (voir les problèmes du tome)",
    page_mismatch: "L'archive ne contient plus cette page à sa place : à rescanner",
    page_corrupt: 'Page abîmée dans le fichier',
    volume_changed: 'Le fichier a changé depuis le scan : à rescanner',
    volume_unavailable: "Aucun fichier connu de ce tome n'existe encore",
    render_failed: 'Impossible de redimensionner cette page',
  },
}

type Widen<T> = { [K in keyof T]: T[K] extends string ? string : Widen<T[K]> }
export type Translations = Widen<typeof fr>
