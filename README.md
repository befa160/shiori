# Shiori

[![CI](https://github.com/befa160/shiori/actions/workflows/ci.yml/badge.svg)](https://github.com/befa160/shiori/actions/workflows/ci.yml)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

Lecteur de mangas, BD et webtoons auto-hébergé. Un seul binaire Rust sert la bibliothèque et une application web installable (PWA), pensée d'abord pour la lecture sur tablette.

> **État : en développement, rien d'utilisable pour l'instant.** La phase 1 (scan d'intégrité des archives) est en cours. L'avancement détaillé est dans [ROADMAP.md](ROADMAP.md).

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
| **Webtoons** | Images très hautes découpées côté serveur, chargement paresseux, aucun saut de mise en page |
| **Bibliothèque** | Grille d'affiches, séries, « Continuer la lecture », recherche, filtres par statut de publication et de lecture |
| **Métadonnées** | ComicInfo.xml, AniList, MangaDex, MangaUpdates, Comic Vine… ; édition manuelle avec verrouillage par champ |
| **Formats** | CBZ (zip, zip64) ; CBR envisagé ensuite. Images jpg, png, webp, avif, servies redimensionnées en WebP/AVIF |
| **Déploiement** | Image Docker et binaires autonomes Linux et Windows |

## Architecture

```
backend/                 workspace Rust
  crates/shiori-core/    archives, identités stables, ordre des pages (sans base ni async)
  crates/shiori/         binaire : CLI de scan, puis serveur web
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

Toute modification passe par une branche et une pull request. La CI vérifie le format, clippy et les tests sous Linux, Windows et macOS, le build du front, et une couverture de tests d'au moins 80 % des lignes et des branches. Les règles complètes sont dans [CLAUDE.md](CLAUDE.md).

## Licence

[MIT](LICENSE)
