import type { Translations } from './fr'

export const en: Translations = {
  common: {
    loading: 'Loading…',
    retry: 'Try again',
  },
  layout: {
    home: 'Shiori, home',
    language: 'Language',
    languageNotSaved: 'Language not saved, try again later',
  },
  notFound: {
    title: 'Page not found',
    home: 'Back to home',
  },
  libraries: {
    title: 'Libraries',
    empty: 'No library yet.',
    emptyHelp: 'Until the administration screen arrives, create one through the API ({{route}}) and start a scan.',
  },
  library: {
    fallbackName: 'Library',
    back: 'Libraries',
    empty: "No series: the library hasn't been scanned yet, or its folder is empty.",
    notFound: 'Library not found',
  },
  series: {
    fallbackName: 'Series',
    back: 'Series',
    empty: 'No volume in this series.',
    notFound: 'Series not found',
  },
  volume: {
    volume_one: 'Volume {{range}}',
    volume_other: 'Volumes {{range}}',
    chapter_one: 'Chapter {{range}}',
    chapter_other: 'Chapters {{range}}',
    special: 'Special',
    specialNumber: 'Special {{range}}',
    unknown: 'No. {{range}}',
    read: 'Read',
    progress: 'Progress of {{label}}',
    statusError: 'Error',
    statusWarning: 'Warning',
  },
  kinds: {
    manga: 'Manga',
    comics: 'Comics',
    webtoon: 'Webtoon',
  },
  counts: {
    series_one: '{{count, number}} series',
    series_other: '{{count, number}} series',
    volumes_one: '{{count, number}} volume',
    volumes_other: '{{count, number}} volumes',
    pages_one: '{{count, number}} page',
    pages_other: '{{count, number}} pages',
  },
  errors: {
    network: 'Server unreachable',
    http_error: 'HTTP error {{status}}',
    internal: 'Internal server error',
    invalid_id: 'Invalid identifier',
    page_broken: 'Unreadable page in the archive (see the volume issues)',
    page_mismatch: 'The archive no longer holds this page in its place: rescan needed',
    page_corrupt: 'Damaged page in the file',
    volume_changed: 'The file changed since the scan: rescan needed',
    volume_unavailable: 'No known file of this volume exists anymore',
    render_failed: 'Unable to resize this page',
  },
}
