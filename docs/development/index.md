# Contribuer

Shiori est une réécriture « clean room » : ne jamais copier ni s'inspirer du code de Kavita (GPL). Les règles complètes sont dans [CLAUDE.md](https://github.com/befa160/shiori/blob/main/CLAUDE.md), la roadmap vivante dans [ROADMAP.md](https://github.com/befa160/shiori/blob/main/ROADMAP.md).

## Prérequis

- Rust stable (fixé par `backend/rust-toolchain.toml`, installé par `rustup`).
- Node.js courant (`web/.node-version`, aujourd'hui 26).

## Commandes

Backend (depuis `backend/`) :

```bash
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p shiori -- serve
```

Front (depuis `web/`) :

```bash
npm ci
npm run dev      # Vite, avec un `shiori serve` lancé à côté
npm run build    # vérification de types comprise
npm run lint
npx vitest run --coverage
npm run e2e      # Playwright contre un vrai serveur (une fois : npx playwright install chromium)
```

Documentation (depuis `docs/`) :

```bash
npm ci
npm run dev      # http://localhost:5173/shiori/
npm run build
```

## Règles de travail

- Une branche et une PR par lot (`feat/…`, `fix/…`, `docs/…`…), jamais de commit direct sur `main`.
- Avant chaque commit : clippy sans avertissement, `cargo fmt` propre, tests verts, build du front.
- Couverture d'au moins 80 % des lignes et des branches, Rust et front ; une branche difficile à atteindre se teste quand même.
- Toujours les dernières versions stables des dépendances.
- Interface : aucun texte en dur (catalogues `fr.ts` et `en.ts`), jamais de couleur en dur, captures aux quatre tailles.
- Documentation : toute page existe en français (racine) et en anglais (`docs/en/`).

## Publier une version

Un tag `vX.Y.Z` sur `main` lance `release.yml` : binaires, archives, `SHA256SUMS`, image Docker `ghcr.io/befa160/shiori`. La documentation est publiée à chaque fusion sur `main` par `docs.yml`.
