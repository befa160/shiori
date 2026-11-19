# Tests, CI et versions

## Tests

- **Rust** : tests unitaires dans des fichiers voisins (`src/foo.rs` déclare `#[cfg(test)] mod tests;` → `src/foo/tests.rs`), tests d'intégration dans `tests/` (vrai serveur, vraie base), fixtures générées par `shiori-testkit` ; `cargo clippy -D warnings`, `cargo fmt`.
- **Web** : Vitest + Testing Library (jsdom), oxlint, vérification de types au build.
- **Bout en bout** : Playwright dans Chromium contre un `shiori serve` de debug sur une bibliothèque générée : invariant de lecture page par page dans chaque mode, reprise, tome suivant, parcours complet, métadonnées, PWA et hors-ligne, captures aux quatre tailles (390×844, 800×1280, 1280×800, 1440×900).
- **Couverture** : au moins **80 % des lignes et des branches**, Rust (`cargo-llvm-cov`, nightly figé) et web (Vitest v8), mesurée sur le code de production ; la CI échoue en dessous.

## CI

- `.github/workflows/ci.yml` : format, clippy et tests sous Linux, Windows et macOS ; couverture ; build et tests du front ; bout en bout ; audit des dépendances.
- `docs.yml` : construit cette documentation et la publie sur GitHub Pages à chaque fusion sur `main`.
- `release.yml` (sur un tag `v*`) : front construit une fois, binaires Linux x86_64 et aarch64 (musl, statiques), Windows x86_64, macOS aarch64 (non signé) ; chaque binaire est lancé et sa version comparée au tag ; archives et `SHA256SUMS` ; image Docker `distroless/static:nonroot` amd64 + arm64 essayée puis poussée sur `ghcr.io/befa160/shiori`. Un tag à suffixe (`v0.1.0-rc.1`) est une préversion.

## Versions

Toujours les **dernières versions stables** : Rust stable (`backend/rust-toolchain.toml`), Node.js courant (`.node-version`), dernières versions des crates, paquets npm et actions GitHub (épinglées par SHA). Dependabot ouvre chaque semaine une PR groupée par écosystème (cargo, npm, actions), une par version majeure. Seules exceptions, figées volontairement : le nightly de la couverture et `cargo-llvm-cov`.
