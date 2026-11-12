# Shiori

Lecteur manga / BD / webtoon auto-hébergé. Backend Rust (axum, tokio, sqlx + SQLite, tracing), un seul binaire qui embarque le front React 19 + TypeScript + Vite + Tailwind + shadcn/ui (rust-embed).

Réécriture « clean room » : ne jamais copier ni s'inspirer du code de Kavita (GPL).

## Structure

- `backend/` : workspace cargo (`crates/*`), toolchain stable fixée par `rust-toolchain.toml`.
- `web/` : application Vite + React + TS.

## Commandes

Backend (depuis `backend/`) :

- Build : `cargo build`
- Tests : `cargo test --workspace`
- Lint : `cargo clippy --workspace --all-targets -- -D warnings`
- Format : `cargo fmt --all` (vérification : `cargo fmt --all --check`)
- Couverture (nightly, la couverture des branches y est encore instable) : `cargo +nightly-2026-11-10 llvm-cov --workspace --branch --summary-only --ignore-filename-regex '(/tests\.rs|/tests/|shiori-testkit/)'`
  (une fois : `rustup toolchain install nightly-2026-11-10 --profile minimal --component llvm-tools`, même date que `NIGHTLY` dans la CI et `cargo install cargo-llvm-cov --locked`)

CLI (depuis `backend/`) :

- Scan d'intégrité : `cargo run -p shiori -- scan <dossier>... [--deep] [--json -] [--no-db]` (aide : `cargo run -p shiori -- scan --help`)
- Serveur HTTP : `cargo run -p shiori -- serve [--listen 127.0.0.1:7878] [--db <fichier>] [--cache-dir <dossier>] [--cache-max-mib 2048] [--jobs N] [--scan-log-days 30]` (API JSON sous `/api`, sans authentification : ne pas écouter sur 0.0.0.0 hors réseau de confiance ; l'interface web est servie hors `/api` : en debug lue dans `web/dist`, en release intégrée au binaire, donc `npm run build` dans `web/` avant `cargo build --release`)
- Base par défaut dans le dossier de données de l'OS ; `SHIORI_DATA_DIR` ou `--db` pour la déplacer. Code de sortie : 0 ok, 1 problème au-delà de `--fail-on`, 2 erreur d'usage ou fatale.

Front (depuis `web/`) :

- Installation : `npm ci`
- Dev : `npm run dev`
- Build (inclut la vérification de types) : `npm run build`
- Lint : `npm run lint` (oxlint)

Avant chaque commit : clippy sans avertissement, `cargo fmt` propre, tests verts, build front OK.

## Versions (obligatoire)

- Toujours les **dernières versions stables** : Rust `stable` (`backend/rust-toolchain.toml`), Node.js courant (`web/.node-version`, aujourd'hui 26, aussi utilisé par la CI), dernières versions des crates, paquets npm et actions GitHub. Pas de support de versions anciennes.
- Ajouter une dépendance = prendre sa dernière version (`cargo info <crate>`, `npm view <paquet> version`) ; `npm outdated` doit rester vide et `cargo update` ne doit rien laisser en retard dans une PR qui touche aux dépendances.
- Seules exceptions, figées volontairement et montées à la main : le nightly de la couverture (`NIGHTLY` dans la CI) et la version de `cargo-llvm-cov`.
- Dependabot (cargo, npm, GitHub Actions) sera mis en place en phase 6 pour maintenir tout cela à jour automatiquement.

## Couverture de tests (obligatoire)

- **Minimum 80 % des lignes ET 80 % des branches**, mesuré sur le code de production. Le job CI `coverage` fait échouer la PR en dessous.
- Les tests unitaires vivent dans un fichier voisin (`src/foo.rs` déclare `#[cfg(test)] mod tests;` → `src/foo/tests.rs`) les tests d'intégration dans `tests/` et les fixtures dans `shiori-testkit` : tout ce code de test est exclu de la mesure, qui ne porte que sur le code testé.
- Une branche difficile à atteindre se teste quand même (archive patchée, erreur construite à la main) ; on ne baisse pas le seuil et on n'exclut pas de code pour passer.
- Front : mêmes seuils avec Vitest (coverage v8, `thresholds.lines` et `thresholds.branches` à 80) dès l'arrivée du premier code front applicatif (phase 3).

## Principes d'architecture (non négociables)

1. Identité stable des pages : ID de page en base = hash(hash du fichier, nom de l'entrée). Jamais d'index brut dans les URLs.
2. URLs d'images immuables (Cache-Control: immutable + ETag). Un fichier modifié produit de nouveaux IDs, donc aucun cache obsolète possible.
3. Le scan est la source de vérité : pour chaque page on stocke id, nom d'entrée, ordre (tri naturel), largeur, hauteur, type. Le lecteur n'infère rien à la volée.
4. Une archive incohérente n'est jamais affichée silencieusement : une page absente ou illisible renvoie une erreur explicite visible dans l'UI, jamais « la dernière image ».
5. Ne jamais modifier les fichiers sources.
6. CBZ : lecture d'une entrée via l'annuaire central (accès direct, pas d'extraction complète), zip64 et gros fichiers gérés. CBR (crate `rars`, pur Rust, jamais unrar) : RAR non solid lu directement par index ; RAR solid (pas d'accès aléatoire) servi depuis un cache d'extraction borné, voir `docs/cbr.md`.
7. Images servies redimensionnées à la demande (largeur cible selon l'appareil, liste fixe de largeurs), en AVIF pour les pages et JPEG pour les miniatures (encodeurs pur Rust), avec cache disque indexé par (page id, paramètres).
8. Identité d'un tome = hash du fichier (renommer ou déplacer ne perd ni progression ni métadonnées). Hash complet au premier passage, puis taille + date de modification pour détecter un changement.
9. Portabilité : le backend compile et passe ses tests sous Linux, Windows et (si possible) macOS, sans changement de code applicatif. Donc :
   - pas de chemins codés en dur (dossiers par OS via la crate `directories`, surchargeables par variables d'environnement / fichier de config) ;
   - aucun appel à des commandes shell externes ;
   - crates pure-Rust plutôt que bibliothèques C ; tout ce qui est natif est isolé derrière un trait et une feature Cargo optionnelle ;
   - noms de fichiers et chemins traités indépendamment des séparateurs de l'OS ;
   - CI GitHub Actions, matrice linux/windows : build, clippy, tests.
   Docker, binaire autonome et éventuel Tauri sont des étapes de distribution, pas des contraintes sur le code.

## Direction UI/UX (hors lecteur)

Le lecteur est plein écran et immersif (barre d'outils masquée, tap au centre pour l'afficher) et il est exclu des règles ci-dessous.

- Thème sombre natif par défaut : fond `#121212`, cartes/panneaux `#1E1E1E`, survol `#2A2A2A`. Accent ambre `#E5A00D` (amber-500) réservé à l'action principale, aux états actifs et aux barres de progression.
- Couvertures en `aspect-[2/3]`, `object-cover`, `rounded-md`. Couverture de repli générée (titre + auteur sur fond dégradé sombre) si absente. Titres tronqués sur 2 lignes (`line-clamp-2`), auteur en `text-zinc-400`.
- Barre de progression fine (`h-1`, amber-500) collée en bas de la couverture pour les tomes en cours. Au survol (desktop) : léger zoom, bordure lumineuse subtile, bouton d'action rapide.
- Navigation : sidebar fixe/rétractable (Bibliothèques, Séries, Statistiques) + recherche globale à partir du breakpoint `lg` ; en dessous, barre d'onglets fixe en bas (4 onglets max). Breakpoint à valider pour la tablette 8" en portrait.
- PWA : zones sûres via `env(safe-area-inset-*)` (les classes `pb-safe`/`pt-safe` ne sont pas natives de Tailwind), bouton « Retour » explicite sur les pages de détail (barre du navigateur masquée en standalone), carrousels sans scrollbar visible mais défilement tactile fluide.
- Vérification visuelle : données de test avec titres très longs et couvertures manquantes ; captures Playwright en 390×844 (mobile), 800×1280 et 1280×800 (tablette 8"), 1440×900 (desktop), et correction des défauts d'alignement.

## Méthode de travail

- `ROADMAP.md` est la roadmap vivante : la lire avant de commencer, la mettre à jour dans chaque PR qui fait avancer une phase (cases, décisions, questions).
- Une branche de travail et une PR par lot (`feat/…`, `fix/…`, `chore/…`, `ci/…`, `docs/…`, `test/…`), jamais de commit direct sur `main`.
- Le graphe Graphify (`graphify-out/`, non versionné) est construit : l'interroger (`graphify query`, `path`, `explain`) avant d'explorer le code pour une question de structure ou d'impact. Les hooks git le tiennent à jour (AST) ; après un changement important des docs, relancer `/graphify . --update`.
- Pour chaque phase : plan détaillé (crates, schéma SQLite, routes API, tâches) validé avant de coder.
- Petits commits, tests à chaque phase.
- Poser les questions bloquantes plutôt que supposer (CBR, formats d'image, breakpoints).
- Préférer les crates éprouvées au code maison, sans jamais sauter les tests d'intégrité, la validation d'entrées ou l'accessibilité.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- When graphify-out/graph.json exists, run `graphify query "<question>"` before your first grep, search or source-file read for a codebase question, and put this rule in every subagent prompt that explores code. Read raw files only for lines the graph or an earlier read already located. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- A committed graphify-out/graph.json is rewritten whole on every build, so a large diff to it in a pull request is expected. Do not flag it as churn or suggest skipping the graph rebuild or its hooks.
