# Roadmap Shiori

Document vivant : il est mis à jour à chaque PR qui fait avancer une phase (cases cochées, décisions, questions résolues).
Légende : ✅ fait · 🚧 en cours · ⏳ à faire · ❓ décision attendue.

| Phase | Contenu | État |
|---|---|---|
| 0 | Outillage (Rust, structure, Ponytail, Graphify, UI/UX Pro Max, MCP shadcn + Playwright) | ✅ |
| 1 | CLI de scan d'intégrité + schéma SQLite + CI linux/windows, puis graphe Graphify | 🚧 plan validé |
| 2 | API axum + streaming/redimensionnement des images + parseur de noms + job de scan | ⏳ |
| 3 | Lecteur web (sens de lecture, webtoon, doubles pages) | ⏳ |
| 4 | Bibliothèque façon Plex + statuts/filtres + DESIGN_SYSTEM.md | ⏳ |
| 5 | Métadonnées web + édition manuelle | ⏳ |
| 6 | Distribution : image Docker, PWA finalisée, binaires Linux/Windows en release, Dependabot | ⏳ |

## Méthode de travail

- **Une branche par lot, une PR par lot**, jamais de commit direct sur `main` (sauf exception validée). Noms de branche : `feat/…`, `fix/…`, `chore/…`, `ci/…`, `docs/…`, `test/…`.
- Chaque PR : description (contexte, changements, tests, captures si UI), CI verte, `cargo fmt` + `cargo clippy -D warnings` propres, **couverture ≥ 80 % lignes et branches** (job `coverage`), ROADMAP.md mis à jour si la PR fait avancer la phase.
- Fusion par toi après relecture ; squash merge par défaut (historique de `main` = une ligne par lot).
- **Graphify** : tant que le graphe n'est pas construit (fin de phase 1), les commits se font avec `GRAPHIFY_SKIP_HOOK=1`. Ensuite les hooks git le maintiennent à jour.
- Avant chaque phase : plan détaillé ici, validation, puis code.

---

## Phase 1 : scan d'intégrité (plan validé le 2026-11-11)

### Objectif

`shiori scan <dossier>` parcourt un dossier, examine chaque CBZ **sans jamais le modifier**, enregistre en base l'identité du tome et de chacune de ses pages, et produit un rapport (JSON + tableau terminal) avec un statut `ok` / `warning` / `error` par fichier.
C'est la fondation des principes 1, 3, 4, 5, 6 et 8 : tout ce que le lecteur affichera plus tard viendra de ce que le scan a établi.

Hors périmètre de la phase 1 : CBR (Q1), serveur HTTP, bibliothèques/séries, parseur de numéros de tome (phase 2), job de scan dans l'app (phase 2, il réutilisera le même moteur).

### Découpage du workspace

```
backend/
  crates/
    shiori-core/     lib : archives, identité, ordre naturel, contrôles d'intégrité, modèle de rapport (aucune I/O base)
    shiori-db/       lib : schéma SQLite (migrations sqlx), persistance des résultats de scan
    shiori-testkit/  lib (dev-dependency) : génération programmatique des fixtures (CBZ sains et cassés)
    shiori/          bin : CLI (clap) ; plus tard le serveur axum dans le même binaire
```

Le crate `shiori` actuel (coquille) devient le binaire. `shiori-core` ne dépend ni de tokio ni de sqlx : il reste testable et réutilisable par le futur job de scan et par le serveur.

### Crates retenues

| Crate | Version | Usage | Pure Rust |
|---|---|---|---|
| `zip` | 9.5 | lecture CBZ via l'annuaire central, accès direct par index, zip64, vérification CRC à la lecture | oui avec `default-features = false` + `deflate-flate2-zlib-rs`, `deflate64`, `lzma`, `xz`, `ppmd` (zstd/bzip2 exclus : bibliothèques C ; une méthode non gérée produit une erreur explicite) |
| `imagesize` | 0.15 | type réel (octets magiques) + dimensions depuis l'en-tête, sans décodage : jpg, png, webp, avif | oui |
| `image` | 0.25 | décodage complet en mode `--deep` (jpg, png, webp) | oui |
| `blake3` | 1.8 | hash du fichier (identité du tome) et des IDs de page, feature `pure` | oui |
| `alphanumeric-sort` | 1.5 | tri naturel (`2.jpg` avant `10.jpg`) | oui |
| `rayon` | 1.12 | scan de plusieurs fichiers en parallèle (`--jobs`) | oui |
| `sqlx` | 0.9 | SQLite, migrations embarquées (`migrate!`), requêtes à l'exécution (pas de `query!` : pas de `DATABASE_URL` à la compilation ni en CI) | **non** : `libsqlite3-sys` compile SQLite (C) via `cc`. Imposé par la stack, compile sous Windows MSVC sans dépendance système |
| `tokio` | 1.54 | runtime pour sqlx (le scan lui-même est synchrone, dans rayon) | oui |
| `clap` | 4.6 | CLI (derive) | oui |
| `serde` / `serde_json` | 1.0 | rapport JSON | oui |
| `comfy-table` | 8.0 | tableau terminal | oui |
| `directories` | 6.0 | dossier de données par OS (surchargeable par `SHIORI_DATA_DIR`) | oui |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | logs (`RUST_LOG`, `-v`) | oui |
| `thiserror` / `anyhow` | 2.0 / 1.0 | erreurs typées dans les libs, `anyhow` dans le binaire | oui |
| `tempfile` | 3.27 | tests (dev-dependency) | oui |

### Identités (principes 1 et 8)

- **Tome** : `volume_id = hex(blake3(contenu complet du fichier))`, 64 caractères. Renommer ou déplacer le fichier ne change rien ; deux copies identiques donnent le même tome.
- **Page** : `page_id = hex(blake3(volume_id_bytes ‖ 0x00 ‖ nom_brut_de_l_entrée)[..16])`, 32 caractères. On utilise les **octets bruts** du nom stocké dans le zip, et non le nom décodé : changer l'heuristique d'encodage (CP437/UTF-8) ne peut pas changer un ID. Avec 128 bits, aucune collision réaliste n'est possible.
- **Détection de changement** : `(chemin, taille, mtime)` → `volume_id`. Si les trois correspondent à la base, on ne recalcule pas le hash (option `--rehash` pour forcer). Sinon : hash complet, et un nouveau contenu donne un nouveau `volume_id`, donc de nouveaux `page_id`. Aucun cache ne peut devenir obsolète.

### Ordre des pages (principe 3)

- Pages = entrées dont le **contenu** est une image reconnue (jpg, png, webp, avif), triées par chemin complet dans l'archive, composante par composante, en tri naturel insensible à la casse, puis par nom brut en départage (ordre total, donc déterministe).
- `ordinal` (0..n-1) est stocké en base. Le lecteur ne trie jamais rien lui-même.
- On garde aussi l'index de l'entrée dans l'annuaire central (`archive_index`) pour l'accès direct en phase 2, et on signale si l'ordre de l'archive diffère de l'ordre naturel.

### Contrôles par fichier et codes de diagnostic

Chaque diagnostic a un code stable (utilisé par l'UI plus tard), une sévérité, éventuellement le nom de l'entrée concernée, et un message.

| Code | Sévérité | Détection |
|---|---|---|
| `archive_unreadable` | error | pas un zip, annuaire central introuvable ou incohérent |
| `archive_truncated` | error | décalage d'une entrée au-delà de la taille du fichier, lecture qui s'arrête avant la fin |
| `encrypted_entry` | error | entrée chiffrée |
| `unsupported_compression` | error | méthode de compression non gérée (zstd, bzip2…) |
| `crc_mismatch` | error | CRC32 calculé ≠ CRC32 annoncé (toutes les entrées sont lues jusqu'au bout) |
| `image_undecodable` | error | en-tête illisible, ou décodage complet en échec avec `--deep` |
| `duplicate_entry_conflict` | error | même nom d'entrée, contenus différents (taille ou CRC) : ambiguïté, donc pas d'ID de page fiable |
| `no_pages` | error | aucune image |
| `duplicate_entry_identical` | warning | même nom, contenu identique : une seule page est conservée |
| `extension_mismatch` | warning | extension ≠ type réel (ex. `.jpg` contenant du png) ; la page reste valide, le type réel fait foi |
| `unsupported_image` | warning | image d'un format hors liste (gif, bmp, tiff…), ignorée comme page (Q3) |
| `non_image_entry` | warning | fichier parasite (txt, nfo, url…) |
| `subdirectories` | warning | pages réparties dans des sous-dossiers |
| `archive_order_differs` | warning | ordre de l'annuaire ≠ ordre naturel (typiquement `1, 10, 2`) |
| `ignored_entry` | info | entrées connues et sans risque : `ComicInfo.xml` (noté pour la phase 5), `__MACOSX/`, `.DS_Store`, `Thumbs.db` |

Statut du fichier = sévérité maximale (info ne dégrade pas `ok`).
**Une page dont l'image est illisible reste listée** à sa place avec `status = broken` : le lecteur affichera une erreur explicite à cet endroit au lieu de sauter la page ou de répéter la précédente (principe 4).

Modes :
- défaut : CRC de toutes les entrées + en-têtes/dimensions de chaque image (rapide, lecture séquentielle unique du fichier) ;
- `--deep` : en plus, décodage complet de chaque image (lent sur un webtoon de 2 Go, mais exhaustif). AVIF : en-tête seulement (Q2).

### Schéma SQLite (migration `0001_scan.sql`)

```sql
CREATE TABLE volumes (
  id              TEXT PRIMARY KEY,          -- blake3 hex du fichier
  size            INTEGER NOT NULL,
  format          TEXT NOT NULL,             -- 'cbz' (plus tard 'cbr')
  status          TEXT NOT NULL CHECK (status IN ('ok','warning','error')),
  page_count      INTEGER NOT NULL,          -- nombre réel de pages (= COUNT(pages))
  scanner_version INTEGER NOT NULL,          -- version des règles de scan ; un changement déclenche un rescan
  scanned_at      TEXT NOT NULL              -- RFC 3339 UTC
);

CREATE TABLE file_locations (                -- où le tome a été vu ; plusieurs chemins possibles
  path         TEXT PRIMARY KEY,             -- chemin absolu normalisé (séparateurs '/')
  volume_id    TEXT NOT NULL REFERENCES volumes(id),
  size         INTEGER NOT NULL,
  mtime_ns     INTEGER NOT NULL,
  last_seen_at TEXT NOT NULL
);
CREATE INDEX file_locations_volume ON file_locations(volume_id);

CREATE TABLE pages (
  id              TEXT PRIMARY KEY,          -- 128 bits hex
  volume_id       TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  ordinal         INTEGER NOT NULL,
  entry_name      TEXT NOT NULL,             -- nom décodé (affichage, diagnostics)
  entry_name_raw  BLOB NOT NULL,             -- octets bruts (identité)
  archive_index   INTEGER NOT NULL,          -- index dans l'annuaire central (accès direct)
  media_type      TEXT,                      -- image/jpeg, image/png, image/webp, image/avif
  width           INTEGER,
  height          INTEGER,
  compressed_size INTEGER NOT NULL,
  size            INTEGER NOT NULL,
  crc32           INTEGER NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('ok','broken')),
  UNIQUE (volume_id, ordinal)
);

CREATE TABLE scan_runs (
  id          INTEGER PRIMARY KEY,
  root        TEXT NOT NULL,
  options     TEXT NOT NULL,                 -- JSON (deep, jobs, rehash…)
  started_at  TEXT NOT NULL,
  finished_at TEXT
);

CREATE TABLE scan_issues (
  id          INTEGER PRIMARY KEY,
  scan_run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
  volume_id   TEXT REFERENCES volumes(id),   -- NULL si le fichier n'a même pas pu être hashé
  path        TEXT NOT NULL,
  severity    TEXT NOT NULL CHECK (severity IN ('info','warning','error')),
  code        TEXT NOT NULL,
  entry_name  TEXT,
  message     TEXT NOT NULL
);
CREATE INDEX scan_issues_volume ON scan_issues(volume_id);
```

Les tables bibliothèques, séries, progression et métadonnées arrivent par migrations dans les phases suivantes ; elles référenceront `volumes.id` et `pages.id`, jamais un chemin ou un index brut.
Les pages d'un tome sont écrites en une transaction ; pour un `volume_id` donné elles sont déterministes, donc un rescan est idempotent.

### CLI

```
shiori scan <DOSSIER>... [--deep] [--jobs N] [--json <FICHIER|->] [--db <FICHIER> | --no-db]
                         [--rehash] [--fail-on error|warning] [-v|-q]
```

- Parcours récursif, extensions `.cbz` (insensible à la casse), liens symboliques non suivis.
- Base par défaut : `<dossier de données OS>/shiori.db` (`SHIORI_DATA_DIR` ou `--db` pour surcharger), `--no-db` pour le rapport seul (Q4).
- Sortie : tableau `fichier | statut | pages | diagnostics` + résumé ; `--json` produit le rapport complet (schéma versionné `report_version: 1`).
- Code de sortie : `0` si aucun fichier n'atteint le seuil `--fail-on` (défaut `error`), `1` sinon, `2` en cas d'erreur d'usage ou d'I/O fatale.
- Fichiers ouverts en lecture seule uniquement ; aucune écriture hors de la base et du fichier JSON demandé.

### Fixtures (`shiori-testkit`)

Générées par du code au moment des tests, sans binaire versionné : déterministes, légères, identiques sous Linux et Windows.

- **Faux webtoon** : 300 pages PNG très hautes (ex. 64×4000 et quelques 800×16000), une couleur par page pour vérifier qu'une page = la bonne image.
- **Faux manga** : couverture, pages simples, doubles pages (ratio > 1,2), 4ème de couverture distincte, `ComicInfo.xml`.
- **Archives cassées** : entrées dupliquées (identiques / conflictuelles, obtenues en patchant les octets du nom), noms non paddés (`1, 10, 2`), sous-dossiers, fichiers parasites, CRC invalide (octet modifié dans une entrée stockée), archive tronquée, entrée chiffrée, extension trompeuse, archive sans image, fichier qui n'est pas un zip.

### Tests

- Unitaires (`shiori-core`) : tri naturel, calcul des IDs (vecteurs fixes), classification des entrées, statut agrégé.
- Intégration : un test par fixture cassée (code et sévérité attendus) ; un test par invariant :
  - **ID de page stable** : deux scans, ou le même fichier copié/renommé ailleurs, donnent des IDs identiques ;
  - **ordre stable** : même ordinal → même entrée sur deux scans ;
  - **jamais de page erronée** : pour chaque page, l'entrée relue par `archive_index` a le CRC enregistré, et sa couleur correspond à l'ordinal attendu (le bug « 4ème de couverture sur 60 pages » est donc détecté) ;
  - **contenu modifié** : un octet ajouté donne un nouveau `volume_id` et de nouveaux `page_id` ;
  - **source intacte** : taille, mtime et hash du fichier inchangés après scan ; scan d'un fichier en lecture seule ;
  - **rescan idempotent** : le deuxième scan ne recalcule pas le hash et ne modifie pas les pages.
- Tous les tests tournent sous Linux et Windows en CI.

### CI (GitHub Actions)

`.github/workflows/ci.yml`, sur `push` vers `main` et sur `pull_request` :
- job `backend`, matrice `ubuntu-latest`, `windows-latest`, `macos-latest` (macOS non bloquant, `continue-on-error`) : `cargo fmt --check` (Linux), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` ; cache `Swatinem/rust-cache`.
- job `web` (Linux) : `npm ci`, `npm run lint`, `npm run build`.
- Protection de `main` (CI requise avant fusion) : réglage GitHub à activer une fois la CI en place.

### Lots (une branche + une PR chacun)

- [x] **1.1** `ci/github-actions` : workflow CI (sur le squelette actuel), badge dans le README. (#2, protection de `main` active : CI requise, historique linéaire)
- [x] **1.2** `feat/core-archive` : `shiori-core` (lecture CBZ, hash du fichier, IDs de page, ordre naturel) + tests unitaires.
- [ ] **1.3** `feat/core-integrity` : contrôles et codes de diagnostic, modèle de rapport, `shiori-testkit` et tests d'intégration sur les fixtures et les invariants.
  - Constat du lot 1.2 : `zip` 9.5 fusionne les entrées de même nom (une seule exposée, `has_duplicate_names()` vrai). Pour distinguer `duplicate_entry_identical` de `duplicate_entry_conflict`, relire l'annuaire central (CRC, tailles) seulement quand ce drapeau est levé.
- [ ] **1.4** `feat/db-schema` : `shiori-db`, migration `0001_scan.sql`, persistance, détection de changement, rescan idempotent.
- [ ] **1.5** `feat/cli-scan` : commande `shiori scan`, tableau, JSON, codes de sortie, dossier de données ; mise à jour des commandes dans CLAUDE.md.
- [ ] **1.6** `chore/graphify-build` : `/graphify .`, vérification des requêtes, fin de `GRAPHIFY_SKIP_HOOK`, bilan de phase ici.

### Questions (résolues le 2026-11-11)

- **Q1 CBR** ✅ : hors phase 1. Avant la phase 2, une note comparera la conversion CBR→CBZ au scan et le cache d'extraction, ainsi que les licences (unrar : licence propriétaire restrictive ; libarchive : BSD mais C, donc derrière une feature).
- **Q2 AVIF en `--deep`** ✅ (proposition retenue) : il n'existe pas de décodeur AVIF pur Rust mature (`image` passe par dav1d, en C). Proposition : en phase 1, AVIF validé par l'en-tête seulement, et décodage complet derrière une feature native optionnelle plus tard. L'encodage AVIF (ravif, pur Rust) et WebP avec perte (libwebp, en C) seront à trancher en phase 2.
- **Q3 GIF/BMP/TIFF** ✅ : avertissement `unsupported_image`, entrée ignorée comme page.
- **Q4 Base par défaut** ✅ : la CLI écrit dans `<dossier de données>/shiori.db` ; `--db` surcharge, `--no-db` produit seulement le rapport.
- **Q5 Doublons identiques** ✅ : avertissement, une seule page conservée ; doublons au contenu différent = erreur.

## Journal des décisions

- 2026-11-11 : dépôt cloné depuis `befa160/shiori` (licence MIT). `graphify-out/` ignoré par git. Hooks Graphify dans `.claude/settings.json` versionné. Playwright MCP en Chromium headless (scope projet).
- 2026-11-11 : travail par branches et PR pour tout le projet.
- 2026-11-11 : plan de la phase 1 validé, propositions Q1 à Q5 retenues.
- 2026-11-11 : toujours les dernières versions stables (Rust stable, Node 26 via `web/.node-version`, dernières crates/paquets/actions), pas de support des versions anciennes. Dependabot (cargo, npm, GitHub Actions) en phase 6.
- 2026-11-11 : couverture minimale 80 % lignes ET branches, imposée par le job CI `coverage` (nightly + `cargo-llvm-cov --branch`, fichiers de tests exclus de la mesure). Tests unitaires déplacés dans des fichiers `tests.rs` voisins.
- 2026-11-11 : `zip` décode en CP437 un nom UTF-8 invalide (pas d'erreur) ; les octets bruts restent la seule base des IDs.
- 2026-11-11 : formule des `page_id` figée par un test d'instantané (`id::tests::page_id_is_frozen`). Ordre naturel : à valeur égale, `9` précède `09` (comportement d'`alphanumeric-sort`, déterministe).
