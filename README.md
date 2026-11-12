# Shiori

[![CI](https://github.com/befa160/shiori/actions/workflows/ci.yml/badge.svg)](https://github.com/befa160/shiori/actions/workflows/ci.yml)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

Lecteur de mangas, BD et webtoons auto-hébergé. Un seul binaire Rust sert la bibliothèque et une application web installable (PWA), pensée d'abord pour la lecture sur tablette.

> **État : en développement (fin de la phase 3).** Utilisables aujourd'hui : `shiori scan` (scan d'intégrité des archives CBZ et CBR), `shiori serve` (API JSON et images) et **le lecteur web** : bibliothèques, séries, lecture manga (droite à gauche), BD (gauche à droite) et webtoon (défilement vertical), double page, zoom, reprise de la lecture, en français et en anglais. La création des bibliothèques se fait encore par l'API (l'interface d'administration arrive en phase 4). L'avancement détaillé est dans [ROADMAP.md](ROADMAP.md).

## Pourquoi

Les lecteurs existants affichent parfois la mauvaise page : un webtoon où la page 1 n'est plus la page 1, un manga qui répète la 4ème de couverture pendant soixante pages. Ces bugs viennent d'un index de page qui ne correspond plus au contenu réel de l'archive (cache périmé, nombre de pages faux).

Shiori est conçu pour que ces défauts soient impossibles :

- **Chaque page a une identité stable** : un hash du fichier et du nom de l'entrée dans l'archive, jamais un simple numéro. Une URL d'image désigne toujours la même image.
- **Le scan est la source de vérité** : l'ordre des pages, leurs dimensions et leur type sont établis une fois au scan et stockés en base. Le lecteur n'invente rien.
- **Une archive abîmée se voit** : une page illisible affiche une erreur explicite à sa place, jamais une autre image.
- **Les fichiers sources ne sont jamais modifiés.**

## Fonctionnalités prévues

| | |
|---|---|
| **Scan d'intégrité** | CRC de chaque entrée, en-têtes et dimensions des images, doublons, fichiers parasites, ordre, archives tronquées ; rapport JSON et dans l'interface |
| **Lecteur** | Gauche→droite, droite→gauche (manga), vertical continu (webtoon) ; page simple ou double ; découpage des doubles pages ; reprise exacte de la lecture |
| **Webtoons** | Défilement vertical continu, chargement paresseux, aucun saut de mise en page (place de chaque page réservée d'après le scan) |
| **Bibliothèque** | Grille d'affiches, séries, « Continuer la lecture », recherche, filtres par statut de publication et de lecture |
| **Métadonnées** | ComicInfo.xml, AniList, MangaDex, MangaUpdates, Comic Vine… ; édition manuelle avec verrouillage par champ |
| **Formats** | CBZ (zip, zip64) et CBR (RAR 4 et 5, solid compris, en Rust pur). Images jpg, png, webp, avif, servies redimensionnées en AVIF (pages) et JPEG (miniatures) |
| **Déploiement** | Image Docker et binaires autonomes Linux et Windows |

## Architecture

```
backend/                 workspace Rust
  crates/shiori-core/    archives, identités stables, ordre des pages, scan d'intégrité (sans base ni async)
  crates/shiori-db/      base SQLite : persistance des scans, détection de changement
  crates/shiori-testkit/ fixtures de test générées par du code
  crates/shiori/         binaire : CLI de scan et serveur HTTP (API, images, job de scan)
web/                     application React 19 + TypeScript + Vite
```

Pile : Rust (axum, tokio, sqlx + SQLite), React + Tailwind + shadcn/ui embarqué dans le binaire. Le code applicatif reste portable : il compile et passe ses tests sous Linux, Windows et macOS, et privilégie les dépendances pure Rust.

## Développement

Prérequis : les dernières versions stables de [Rust](https://rustup.rs) (canal `stable`, fixé par `backend/rust-toolchain.toml`) et de [Node.js](https://nodejs.org) (26, fixé par `web/.node-version`). Le projet suit toujours les dernières versions de ses outils et dépendances, pour que la maintenance reste simple.

```bash
cd backend
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

```bash
cd web
npm ci
npm run dev
```

Scanner une bibliothèque (rapport dans le terminal, base SQLite dans le dossier de données de l'OS, ou `SHIORI_DATA_DIR`) :

```bash
cd backend
cargo run --release -p shiori -- scan /chemin/vers/ma/bibliotheque
```

Options utiles : `--deep` (décode chaque image), `--json rapport.json` (rapport complet), `--no-db` (sans base), `--fail-on warning`.

Lancer Shiori (interface web et API sous `/api`, sans authentification, écoute sur `127.0.0.1:7878` par défaut). Le front est intégré au binaire de release : le construire d'abord.

```bash
cd web && npm ci && npm run build
```

```bash
cd backend && cargo run --release -p shiori -- serve
```

Puis, en attendant l'interface d'administration (phase 4), déclarer une bibliothèque (un dossier par série ; `kind` : `manga`, `comics` ou `webtoon`, qui donne le sens de lecture par défaut) et la scanner :

```bash
curl -X POST localhost:7878/api/libraries -H 'content-type: application/json' -d '{"name":"Manga","root":"/chemin/vers/mes/mangas","kind":"manga"}'
```

```bash
curl -X POST localhost:7878/api/admin/scans -H 'content-type: application/json' -d '{"library_id":1}'
```

(`1` : l'`id` renvoyé par la commande précédente, à adapter pour chaque bibliothèque.)

et ouvrir <http://localhost:7878>. Dans le lecteur : tap sur les côtés ou flèches pour tourner les pages, tap au centre (ou Tab) pour les commandes, double tap pour zoomer ; le panneau « Affichage » règle le sens de lecture et la disposition, retenus pour la série.

Tests du front : `npm test` (Vitest) et `npm run e2e` (le lecteur dans Chromium contre un vrai serveur, sur une bibliothèque générée ; une fois : `npx playwright install chromium`).

Toute modification passe par une branche et une pull request. La CI vérifie le format, clippy et les tests sous Linux, Windows et macOS, le build du front, le lecteur de bout en bout dans Chromium, et une couverture de tests d'au moins 80 % des lignes et des branches. Les règles complètes sont dans [CLAUDE.md](CLAUDE.md).

## Licence

[MIT](LICENSE)
