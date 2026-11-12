# Roadmap Shiori

Document vivant : il est mis à jour à chaque PR qui fait avancer une phase (cases cochées, décisions, questions résolues).
Légende : ✅ fait · 🚧 en cours · ⏳ à faire · ❓ décision attendue.

| Phase | Contenu | État |
|---|---|---|
| 0 | Outillage (Rust, structure, Ponytail, Graphify, UI/UX Pro Max, MCP shadcn + Playwright) | ✅ |
| 1 | CLI de scan d'intégrité + schéma SQLite + CI linux/windows, puis graphe Graphify | ✅ |
| 2 | API axum + streaming/redimensionnement des images + parseur de noms + job de scan + API d'administration des scans + CBR | ✅ |
| 3 | Lecteur web (sens de lecture, webtoon, doubles pages) + front embarqué + progression de lecture | 🚧 plan validé |
| 4 | Bibliothèque façon Plex + statuts/filtres + menu d'administration (scans, journaux) + DESIGN_SYSTEM.md | ⏳ |
| 5 | Métadonnées web + édition manuelle | ⏳ |
| 6 | Distribution : image Docker, PWA finalisée, binaires Linux/Windows en release, Dependabot, documentation (technique, installation, fonctionnelle) | ⏳ |

## Méthode de travail

- **Une branche par lot, une PR par lot**, jamais de commit direct sur `main` (sauf exception validée). Noms de branche : `feat/…`, `fix/…`, `chore/…`, `ci/…`, `docs/…`, `test/…`.
- Chaque PR : description (contexte, changements, tests, captures si UI), CI verte, `cargo fmt` + `cargo clippy -D warnings` propres, **couverture ≥ 80 % lignes et branches** (job `coverage`), ROADMAP.md mis à jour si la PR fait avancer la phase.
- Fusion par toi après relecture ; squash merge par défaut (historique de `main` = une ligne par lot).
- **Graphify** : graphe construit à la fin de la phase 1 ; les hooks git le maintiennent à jour, plus de `GRAPHIFY_SKIP_HOOK`.
- Avant chaque phase : plan détaillé ici, validation, puis code.

---

## Phase 1 : scan d'intégrité (plan validé le 2026-11-11, terminée le 2026-11-11)

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
| `size_mismatch` | error | l'entrée contient plus d'octets que sa taille déclarée (ajouté au lot 1.3) |
| `entry_too_large` | error | taille déclarée > 512 Mio, refusée avant lecture : garde-fou contre les bombes zip (ajouté au lot 1.3) |
| `file_changed_during_scan` | error | taille ou date de modification changée entre le hash et la lecture des entrées : pages écartées, à rescanner (ajouté au lot 1.3, review Ponytail) |
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
- défaut : CRC de toutes les entrées + en-têtes/dimensions de chaque image (le fichier est lu deux fois : hash complet, puis entrées ; le rescan évite le hash, lot 1.4) ;
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
- [x] **1.3** `feat/core-integrity` : contrôles et codes de diagnostic, modèle de rapport, `shiori-testkit` et tests d'intégration sur les fixtures et les invariants.
  - `Archive::duplicates()` relit l'annuaire central quand `zip` signale des doublons (CRC + tailles 32 bits comparés).
  - Une archive qui commence par `PK\x03\x04` mais dont l'annuaire est introuvable est classée `archive_truncated`.
  - **Essai réel** (webtoon de 1,96 Go, 12 760 pages déjà découpées en 800×1280 max, 116 chapitres) :
    - `archive_order_differs` : 7 440 pages hors de leur place dans l'archive (paires voisines inversées, typique d'un téléchargement parallèle). Piste plausible pour les sauts de page constatés dans Kavita ; Shiori applique l'ordre naturel.
    - Hash : 34 Mo/s à froid (disque), 836 Mo/s en cache. Scan normal 10,5 s, `--deep` 248 s (décodage séquentiel), mémoire max 12,6 Mo (19 Mo en `deep`).
    - Conséquences : la double lecture n'est pas un problème ; 12 760 lignes de pages à écrire en une transaction (1.4) ; liste virtualisée dans le lecteur (phase 3) ; `index.json` d'outil de téléchargement (titre, auteurs, description, URL) comme source de métadonnées locale possible (phase 5).
    - Amélioration possible, non prioritaire : décoder les pages en parallèle en mode `--deep`.
- [x] **1.4** `feat/db-schema` : `shiori-db`, migration `0001_scan.sql`, persistance, détection de changement, rescan idempotent.
  - `Db::scan` : fichier inchangé (chemin, taille, mtime, `SCANNER_VERSION`) → rapport relu depuis la base, ni hash ni rescan ; sinon `scan_file` dans `spawn_blocking` puis écriture en une transaction.
  - Les pages sont mises à jour **sur place** par leur ID (ordinaux décalés le temps de la mise à jour), jamais supprimées puis réinsérées : les futures tables qui référencent `pages.id` ne perdent rien en cascade. Seules les pages disparues de l'archive sont supprimées.
  - `scan_issues` = état courant (problèmes du dernier scan de chaque tome, ou du chemin si aucun tome fiable). Un fichier absent, illisible avant hash ou modifié pendant le scan n'a pas de `file_locations` : il est retenté au scan suivant.
  - Un fichier modifié donne un nouveau tome ; l'ancien reste en base (progression future préservée). Le nettoyage des tomes orphelins viendra avec les bibliothèques (phase 2).
  - Noms des enums en base = noms serde du cœur (une seule source de vérité). SQL toujours statique (`concat!`), exigé par sqlx 0.9.
  - 12 760 pages (taille de l'essai réel) : écriture en ~1,4 s en debug, relu identique à l'écrit.
- [x] **1.5** `feat/cli-scan` : commande `shiori scan`, tableau, JSON, codes de sortie, dossier de données ; mise à jour des commandes dans CLAUDE.md.
  - Parcours récursif trié et dédoublonné, `.cbz` insensible à la casse, liens symboliques non suivis dans les dossiers ; un fichier passé explicitement est toujours scanné ; un sous-dossier illisible est signalé et ignoré.
  - Parallélisme : `--jobs` (défaut : nombre de cœurs), sémaphore tokio ; résultats remis dans l'ordre des fichiers (même JSON avec `--jobs 1` et `--jobs 8`).
  - JSON `report_version: 1` : options, résumé, fichiers (rapport complet + `skipped`). Avec `--json -`, le tableau passe sur la sortie d'erreur.
  - L'exemple local `examples/scan.rs` de l'essai réel est supprimé : la CLI le remplace.
- [x] **1.6** `chore/graphify-build` : `/graphify .`, vérification des requêtes, fin de `GRAPHIFY_SKIP_HOOK`, bilan de phase ici.
  - Graphe : 530 nœuds, 903 arêtes, 21 communautés (code par AST, sans LLM ; `CLAUDE.md`, `README.md`, `ROADMAP.md` et `ci.yml` par extraction sémantique ; images et README du gabarit Vite écartés). Nœuds centraux : `IssueCode`, `FileReport`, `write_zip`, `ArchiveError`, `Db`.
  - Limites connues : 150 arêtes vers des symboles externes (std, crates) sans nœud ; `0001_scan.sql` non analysé (grammaire SQL absente, `uv tool install "graphifyy[sql]"` pour l'ajouter).
  - `every_page_is_the_right_image` couvre aussi une archive rangée dans le désordre (`unpadded`), le cas du webtoon réel.

### Bilan de la phase 1

- Livré : `shiori-core` (archives CBZ, identités stables, ordre naturel, scan d'intégrité, 18 codes), `shiori-db` (SQLite, rescan sans hash, pages mises à jour sur place), `shiori-testkit` (fixtures générées), CLI `shiori scan`, CI Linux / Windows / macOS avec couverture obligatoire ≥ 80 % (lignes 95 %, branches 90 % en fin de phase).
- Validé sur un vrai webtoon de 2 Go (12 760 pages, archive en désordre) : ordre de lecture correct, 19 Mo de mémoire, rescan instantané.
- Reste ouvert pour la suite : note CBR (Q1) avant la phase 2 ; décodage parallèle en `--deep` (non prioritaire) ; nettoyage des tomes orphelins avec les bibliothèques.

### Questions (résolues le 2026-11-11)

- **Q1 CBR** ✅ : hors phase 1. Avant la phase 2, une note comparera la conversion CBR→CBZ au scan et le cache d'extraction, ainsi que les licences (unrar : licence propriétaire restrictive ; libarchive : BSD mais C, donc derrière une feature). Note rédigée et tranchée : [docs/cbr.md](docs/cbr.md).
- **Q2 AVIF en `--deep`** ✅ (proposition retenue) : il n'existe pas de décodeur AVIF pur Rust mature (`image` passe par dav1d, en C). Proposition : en phase 1, AVIF validé par l'en-tête seulement, et décodage complet derrière une feature native optionnelle plus tard. Encodage tranché en phase 2 (Q6) : AVIF (`ravif`) + JPEG.
- **Q3 GIF/BMP/TIFF** ✅ : avertissement `unsupported_image`, entrée ignorée comme page.
- **Q4 Base par défaut** ✅ : la CLI écrit dans `<dossier de données>/shiori.db` ; `--db` surcharge, `--no-db` produit seulement le rapport.
- **Q5 Doublons identiques** ✅ : avertissement, une seule page conservée ; doublons au contenu différent = erreur.

## Phase 2 : serveur, images, bibliothèques et scans (plan validé le 2026-11-11, terminée le 2026-11-12)

### Objectif

`shiori serve` démarre un serveur HTTP qui expose les bibliothèques, séries, tomes et pages établis par le scan, sert chaque page par son ID (originale ou redimensionnée, URL immuable) et pilote les scans (lancer, suivre, annuler, historique, journaux).
C'est l'API dont le lecteur (phase 3) et la bibliothèque (phase 4) auront besoin ; aucune interface web dans cette phase.

Hors périmètre : front embarqué (`rust-embed`, phase 3), authentification et rôles (« Plus tard »), progression de lecture (phase 3), métadonnées web (phase 5), scans planifiés ou surveillance du disque (plus tard), push de la progression par SSE/WebSocket (le front de la phase 4 interrogera l'API). Le CBR est dans le périmètre (lot 2.8, [note CBR](docs/cbr.md)).

### Découpage du workspace

Pas de nouveau crate : le crate `shiori` gagne une cible lib (`src/lib.rs`, le `main.rs` ne fait qu'appeler `run`), ce qui permet les tests HTTP dans `tests/` et laisse le parcours de dossiers de `scan.rs` réutilisable par le job.

```
shiori-core   + name.rs        parseur de noms de tomes (fonction pure)
              + archive/rar.rs (lot 2.8, `rars` pur Rust, sans feature)
shiori-db     + migrations 0002, 0003 ; bibliothèques, séries, emplacements, passes de scan, journaux
shiori        + lib : server/ (routes axum, erreurs, état), render.rs (redimensionnement), cache.rs (cache disque), job.rs (job de scan)
              + commande `serve`
```

### Crates ajoutées

| Crate | Version | Usage | Pure Rust |
|---|---|---|---|
| `axum` | 0.8.9 | routes HTTP (features `http1`, `json`, `query`, `tokio`, `tracing`) | oui |
| `tower-http` | 0.7.1 | `TraceLayer` (journal des requêtes) | oui |
| `tower` / `http-body-util` | 0.5.3 / 0.1.5 | tests : `ServiceExt::oneshot` sur le routeur, sans réseau (dev-dependencies) | oui |
| `image` (existante) | 0.25 | décodage, `imageops::resize` (Lanczos3), encodage JPEG | oui |
| `ravif` | 0.13 | encodage AVIF des pages, sans la feature `asm` (qui exige `nasm`) | oui |
| `rars` | 0.12.4 | lecture RAR (accès direct par index, plafonds de sortie, annulation), `default-features = false` ; pur Rust, donc sans feature Cargo | oui |

Pas de `tokio-util` (un `AtomicBool` suffit pour l'annulation), pas de `mime_guess` (le type réel est en base), pas de `fast_image_resize` tant que la mesure sur le webtoon réel ne montre pas que `imageops` est trop lent.

### Migrations SQLite

**`0002_libraries.sql`**

```sql
CREATE TABLE libraries (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  root       TEXT NOT NULL UNIQUE,        -- chemin absolu canonique, séparateurs '/'
  created_at TEXT NOT NULL
);

CREATE TABLE series (
  id         INTEGER PRIMARY KEY,
  library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,               -- nom du dossier de série (parseur pour les fichiers à la racine)
  sort_name  TEXT NOT NULL,               -- clé de tri (minuscules, articles retirés plus tard)
  UNIQUE (library_id, name)
);

-- Ce qui dépend du chemin vit sur l'emplacement, pas sur le tome (un tome peut être vu à plusieurs endroits).
ALTER TABLE file_locations ADD COLUMN library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE;
ALTER TABLE file_locations ADD COLUMN series_id  INTEGER REFERENCES series(id) ON DELETE SET NULL;
ALTER TABLE file_locations ADD COLUMN kind       TEXT CHECK (kind IN ('volume','chapter','special','unknown'));
ALTER TABLE file_locations ADD COLUMN number     REAL;   -- 1, 1.5 ; NULL si absent
ALTER TABLE file_locations ADD COLUMN number_end REAL;   -- « Tomes 1-3 »
ALTER TABLE file_locations ADD COLUMN parser_version INTEGER;  -- un changement de règles relance l'analyse sans rehash
CREATE INDEX file_locations_series ON file_locations(series_id);
```

- `library_id` NULL = fichier scanné par la CLI hors de toute bibliothèque (comportement actuel conservé).
- Suppression d'une bibliothèque : ses emplacements et séries disparaissent, **les tomes et pages restent** (progression future préservée, principe 8) et réapparaissent si le fichier est rescanné ailleurs.
- Identité des séries : (bibliothèque, nom). Renommer le dossier d'une série crée une nouvelle série ; acceptable tant qu'il n'y a pas de métadonnées éditées (phase 5 à revoir).

**`0003_scan_jobs.sql`**

```sql
ALTER TABLE scan_runs ADD COLUMN library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL;
ALTER TABLE scan_runs ADD COLUMN origin     TEXT NOT NULL DEFAULT 'cli' CHECK (origin IN ('cli','api'));
ALTER TABLE scan_runs ADD COLUMN status     TEXT NOT NULL DEFAULT 'completed'
  CHECK (status IN ('running','completed','cancelled','failed','interrupted'));
ALTER TABLE scan_runs ADD COLUMN files_total     INTEGER;
ALTER TABLE scan_runs ADD COLUMN files_ok        INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_warning   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_error     INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_unchanged INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_removed   INTEGER NOT NULL DEFAULT 0;  -- emplacements disparus
ALTER TABLE scan_runs ADD COLUMN error           TEXT;                        -- erreur fatale
UPDATE scan_runs SET status = 'interrupted' WHERE finished_at IS NULL;      -- passes de phase 1 coupées

CREATE TABLE scan_run_logs (
  id          INTEGER PRIMARY KEY,
  scan_run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
  at          TEXT NOT NULL,
  level       TEXT NOT NULL CHECK (level IN ('info','warning','error')),
  path        TEXT,
  code        TEXT,                        -- code de diagnostic quand il y en a un
  message     TEXT NOT NULL
);
CREATE INDEX scan_run_logs_run ON scan_run_logs(scan_run_id, id);
```

- `scan_issues` reste l'état courant par tome ; `scan_run_logs` est l'historique d'une passe (fichier scanné, inchangé, disparu, chaque diagnostic, erreur fatale).
- Le code écrit toujours `status` explicitement (le défaut ne sert qu'aux lignes existantes).
- Au démarrage du serveur, une passe restée `running` (arrêt brutal) passe à `interrupted`.
- Conservation : passes et journaux de plus de `--scan-log-days` jours (défaut 30) supprimés au démarrage et à la fin de chaque passe.

### Parseur de noms de tomes (`shiori-core::name`)

Fonction pure `parse(relative_path) -> ParsedName { series, kind, number, number_end }`, sans I/O, testée par une table de cas (fichier de cas dans `name/tests.rs`).

- Série : premier dossier sous la racine de la bibliothèque (`Bibliothèque/One Piece/One Piece T01.cbz` → `One Piece`) ; fichier posé à la racine → partie du nom avant le marqueur de numéro.
- Marqueurs reconnus, insensibles à la casse, avec ou sans point ni espace : FR `Tome`, `T`, `Vol`, `Volume`, `Chapitre`, `Ch`, `HS`/`Hors-série`, `Intégrale` ; EN `Volume`, `Vol`, `v`, `Chapter`, `Ch`, `c`, `#`, `Special`, `Extra`, `Omake`, `One-shot` ; JA `第N巻` (tome), `第N話` (chapitre).
- Numéros : entiers paddés ou non, décimaux (`1.5`, `1,5`), intervalles (`1-3`, `01 à 03`) ; nombre nu final en dernier recours (`Blame 03.cbz` → tome 3). Les tags entre crochets ou parenthèses (`[Team]`, `(2019)`, `(Digital)`) sont ignorés ; une année seule n'est jamais un numéro.
- Ordre dans une série : tomes, puis chapitres hors tome, puis spéciaux, puis `unknown` ; à l'intérieur, `number`, puis `number_end`, puis tri naturel du nom de fichier (ordre total, comme les pages).
- `PARSER_VERSION` figée par un test ; la changer relance l'analyse des emplacements au scan suivant, sans rehash.

### Job de scan (`shiori::job`)

- Un seul scan à la fois pour tout le serveur ; une demande pendant un scan renvoie `409 scan_already_running` (pas de file d'attente : à ajouter si le besoin apparaît).
- Moteur : le même que la CLI (`Db::scan` fichier par fichier, parallélisme `--jobs`), dans une tâche tokio. Cible : une bibliothèque, un dossier ou un fichier **situés sous la racine d'une bibliothèque** (refus sinon : l'API ne doit pas permettre de lire n'importe quel dossier du serveur).
- Étapes : parcours (`files_total` connu avant de scanner), scan de chaque fichier, analyse du nom (parseur) et rattachement à la série, puis, pour une bibliothèque ou un dossier complet, suppression des emplacements non revus (`files_removed`) et des séries vides.
- Progression en mémoire (`RwLock` : passe, fichiers faits / total, fichier courant, compteurs, début) ; compteurs écrits en base à la fin.
- Annulation : drapeau vérifié entre deux fichiers ; le fichier en cours se termine (sa transaction reste cohérente), la passe finit `cancelled`, rien n'est supprimé pour les fichiers non visités.
- La CLI `shiori scan` écrit aussi ses passes (`origin = 'cli'`, compteurs, journal). SQLite est déjà en WAL avec `busy_timeout` : CLI et serveur peuvent tourner en même temps.

### Images (principes 2 et 7)

- **Originale** : `GET /api/pages/{page_id}/image` lit l'entrée par son `archive_index` et la renvoie telle quelle (`Content-Type` = type réel en base). Avant de servir, le serveur vérifie que l'entrée à cet index porte bien le nom brut et le CRC enregistrés ; le CRC est revérifié à la lecture.
- **Redimensionnée** : `GET /api/pages/{page_id}/image?w=<largeur>&f=<format>`.
  - Largeurs autorisées seulement : `160, 320, 480, 640, 800, 1080, 1280, 1600, 2048` (`400 invalid_width` sinon) ; le client choisit la largeur immédiatement supérieure à `largeur CSS × devicePixelRatio`. Une liste fixe borne le cache et empêche de faire calculer des milliers de variantes.
  - Si `w` ≥ largeur d'origine, l'originale est servie sans réencodage (aucune perte, aucun calcul). Le webtoon réel (800×1280) est dans ce cas sur la plupart des écrans.
  - Le format fait partie de l'URL, jamais de négociation par `Accept` : une URL = un seul contenu, pas de `Vary`.
  - Formats : `f=avif` (pages, `ravif`) ou `f=jpeg` (miniatures, couvertures) ; rien d'autre (`400 invalid_format`). Vitesse d'encodage AVIF à mesurer au lot 2.5 sur le webtoon réel et sur Toriko.
  - Sources AVIF : pas de décodeur pur Rust, donc servies originales, sans redimensionnement (décodage derrière une feature native plus tard, cf. Q2).
- **En-têtes** : `Cache-Control: public, max-age=31536000, immutable`, `ETag` fort (`"<page_id>-<w>-<f>-<RENDER_VERSION>"`), `304` sur `If-None-Match`. Un fichier modifié donne de nouveaux `page_id`, donc de nouvelles URL : rien à invalider.
- **Cache disque** : `<cache>/pages/<2 premiers caractères>/<page_id>-w<w>.<f>`, dossier de cache de l'OS (`directories`), surchargeable par `SHIORI_CACHE_DIR` / `--cache-dir`. Écriture dans un fichier temporaire puis renommage atomique (deux requêtes simultanées calculent deux fois mais n'écrivent jamais un fichier partiel). Taille maximale `--cache-max` (défaut 2 Gio, partagée avec les tomes RAR solid extraits) ; au-delà, suppression des fichiers les plus anciens (date de modification, rafraîchie à chaque lecture) par une tâche de fond.
- **Charge** : décodage et encodage dans `spawn_blocking`, limités par un sémaphore (nombre de cœurs) ; plafond de pixels décodés (garde-fou contre une image piégée) ; `catch_unwind` autour des décodeurs, comme au scan.
- **Erreurs explicites (principe 4)**, en JSON `{ "code", "message" }` :

| Code | HTTP | Cas |
|---|---|---|
| `page_not_found` | 404 | ID inconnu |
| `page_broken` | 422 | page marquée `broken` au scan |
| `volume_unavailable` | 410 | aucun emplacement connu du tome n'existe encore sur le disque |
| `volume_changed` | 409 | taille ou date du fichier différente du scan : à rescanner (aucune autre page servie à la place) |
| `page_mismatch` | 409 | l'entrée à l'`archive_index` n'a plus le nom ou le CRC enregistré |
| `page_corrupt` | 500 | CRC faux à la lecture |
| `render_failed` | 500 | décodage ou encodage en échec |

- Mesure à faire au lot 2.4 sur le webtoon réel : temps d'ouverture de l'annuaire central (12 760 entrées) à chaque requête. S'il dépasse quelques millisecondes, garder en mémoire quelques annuaires ouverts (LRU), sinon rien.

### Routes API

Toutes en JSON sous `/api`, erreurs au format `{ "code", "message" }`, listes paginées par `?limit=&after=` quand elles peuvent être longues.

| Méthode | Route | Réponse |
|---|---|---|
| GET | `/api/health` | `{ "status": "ok", "version" }` |
| GET | `/api/libraries` | bibliothèques avec nombre de séries et de tomes |
| POST | `/api/libraries` | `{ name, root }` → 201 ; `root` doit exister et être un dossier (`invalid_root`), unique |
| DELETE | `/api/libraries/{id}` | 204 ; ne touche à aucun fichier, conserve les tomes |
| GET | `/api/libraries/{id}/series` | séries triées par `sort_name`, avec nombre de tomes et ID de la couverture (page 0 du premier tome) |
| GET | `/api/series/{id}` | série + tomes dans l'ordre du parseur (`volume_id`, `kind`, `number`, `status`, `page_count`, couverture) |
| GET | `/api/volumes/{volume_id}` | tome : statut, format, taille, emplacements, problèmes du dernier scan |
| GET | `/api/volumes/{volume_id}/pages` | pages par `ordinal` : `{ id, ordinal, width, height, media_type, status }` ; le lecteur n'en déduit jamais rien d'autre |
| GET | `/api/pages/{page_id}/image` | originale, voir ci-dessus |
| GET | `/api/pages/{page_id}/image?w=&f=` | redimensionnée, voir ci-dessus |
| POST | `/api/admin/scans` | `{ library_id } \| { path }` + `{ deep, rehash }` → 202 + passe ; 409 si un scan tourne, 400 si le chemin est hors bibliothèque |
| GET | `/api/admin/scans/current` | progression de la passe en cours, ou 204 |
| POST | `/api/admin/scans/current/cancel` | 202 ; 404 si rien ne tourne |
| GET | `/api/admin/scans` | historique (filtres `status`, `library_id`), du plus récent au plus ancien |
| GET | `/api/admin/scans/{id}` | détail : options, durée, compteurs, erreur fatale |
| GET | `/api/admin/scans/{id}/logs` | journal paginé (filtre `level`) |

Sécurité tant qu'il n'y a pas d'authentification : écoute sur `127.0.0.1:7878` par défaut, `--listen 0.0.0.0:7878` pour ouvrir au réseau local, en connaissance de cause (Q10). Validation stricte des entrées : IDs en hexadécimal de longueur fixe (`ParseIdError` existe déjà), largeurs et formats en liste fermée, chemins canonisés et confinés aux racines des bibliothèques.

### Commande `serve`

```
shiori serve [--listen 127.0.0.1:7878] [--db <FICHIER>] [--cache-dir <DOSSIER>] [--cache-max 2GiB]
             [--jobs N] [--scan-log-days 30] [-v|-q]
```

Arrêt propre sur Ctrl-C (`with_graceful_shutdown`) : un scan en cours est annulé et sa passe close `cancelled`.

### Tests

- Parseur : table d'au moins 80 noms réels ou réalistes (FR, EN, JA, tags, années, décimaux, intervalles, noms sans numéro), `PARSER_VERSION` figée.
- Base : migrations sur une base de phase 1 existante (les données de la phase 1 survivent), cascade de suppression d'une bibliothèque (tomes conservés), purge des journaux.
- HTTP (routeur en mémoire via `oneshot`, fixtures `shiori-testkit` scannées dans une base temporaire) :
  - **invariant d'ordre de lecture** : sur l'archive rangée dans le désordre (`unpadded`) et sur le faux webtoon, pour chaque `ordinal` *n* de `/pages`, l'image servie par `/api/pages/{id}/image` se décode et a la couleur `page_color(n)` ; même contrôle sur la version redimensionnée ;
  - en-têtes `Cache-Control` / `ETag`, `304` sur `If-None-Match` ;
  - chaque code d'erreur du tableau provoqué volontairement (page cassée, fichier modifié après scan, fichier supprimé, entrée remplacée à taille et date identiques, CRC faux) ;
  - migration `0003` : une passe de phase 1 sans `finished_at` devient `interrupted` ;
  - largeur ou format hors liste → 400 ; ID mal formé → 400 ; chemin de scan hors bibliothèque → 400 ;
  - job : progression, annulation, 409 sur double lancement, passe `interrupted` après redémarrage, emplacements disparus retirés.
- Essai réel en fin de phase sur `~/webtoon-test.cbz` : temps de réponse d'une page originale et redimensionnée (à froid et en cache), taille des variantes, mémoire du serveur.

### Lots (une branche + une PR chacun)

- [x] **2.1** `feat/core-name-parser` : `shiori-core::name`, table de cas, `PARSER_VERSION`.
  - `parse(chemin)` → `ParsedName { folder, series, kind, number, number_end }` et `compare_in_series` (type, numéro, intervalle, puis `compare_entry_names` : ordre total). Une seule dépendance ajoutée : `regex` 1.13 (pur Rust).
  - Priorité des règles : spécial, `第N巻`, `第N話`, tome, chapitre, nombre nu final, nombre nu initial. Un nom qui porte tome et chapitre (`v01 c001`) est un tome. Les points hors décimaux et `_` deviennent des espaces (`One.Piece.v01`).
  - Année (19xx, 20xx) jamais prise pour un nombre nu ; avec un marqueur explicite (`T2019`) elle reste un numéro. Limite connue : chiffres pleine chasse (`第１巻`) non reconnus (`shortcut:` dans le code).
  - Un marqueur spécial (`Extra`, `Special`, `HS`…) ne compte que suivi d'un numéro ou en fin de nom : `Extra Life T01` est un tome (review Ponytail).
  - **Essai sur la vraie bibliothèque** (450 CBZ/CBR d'un dossier de téléchargements, 2026-11-11) : les dossiers ont des noms bruités (`Birdmen (01-16) (Tanabe) (2020) [Digital-2100] (TONER-PapriKa+)`). D'où la séparation entre `folder` (premier dossier tel quel, clé de regroupement, Q9) et `series` (nom affiché : ce qui précède le numéro dans le nom du fichier, à défaut le dossier nettoyé sans sa plage de tomes, l'écriture du dossier gagnant à la casse près). Résultat : un seul nom lisible par dossier sur les 34 séries (`Vinland Saga`, `Birdmen`, `World's End Harem`…). Un tag qui ne contient qu'un numéro (`(T39)`) est gardé.
  - Constaté pour le lot 2.2 : environ 60 fichiers sont posés à la racine (`Wind Breaker`, `Berserk`…), et `Berserk` existe à la fois en dossier et à la racine. Proposition : regrouper les fichiers de la racine par nom affiché, et rattacher un fichier de la racine à la série-dossier qui porte le même nom affiché. Nom stocké d'une série = `series` de son premier fichier dans l'ordre `compare_in_series` (déterministe d'un scan à l'autre).
  - 110 cas dans la table, dont 18 noms réels ; `PARSER_VERSION` = 1 figée par un instantané des résultats de la table (le test casse si un résultat change sans incrément).
- [x] **2.2** `feat/db-libraries` : migration `0002`, API `Db` des bibliothèques, séries et emplacements analysés ; rattachement au scan ; suppression des emplacements disparus.
  - Écarts au plan, suite à l'essai sur la vraie bibliothèque (lot 2.1) : `series` a une colonne `folder` (clé de regroupement : index unique `(library_id, folder)`, et `(library_id, name)` pour les fichiers de la racine) au lieu de `UNIQUE (library_id, name)` ; `sort_name` et `file_locations.parser_version` retirés : le job rattachera chaque fichier à chaque passe (analyse du nom gratuite), donc aucune version à suivre, et le tri des séries se fera à l'affichage.
  - `Db` : `create_library` (nom non vide et unique, racine = dossier existant, racines qui se chevauchent refusées : un fichier n'appartient qu'à une bibliothèque), `libraries`, `delete_library` (emplacements et séries supprimés, tomes et pages conservés), `attach` (emplacement → série, type, numéro), `remove_missing` (emplacements que la passe n'a pas vus, limités à un dossier ; colonne `file_locations.last_seen_run` : un numéro de passe et non un horodatage, deux passes pouvant tomber dans la même milliseconde, défaut trouvé par la CI), `refresh_series` (nom = premier fichier dans l'ordre de la série, fichiers de la racine fusionnés dans la série-dossier de même nom, séries vides supprimées).
  - Un fichier illisible a un tome en erreur : il est rattaché à sa série pour que l'erreur se voie (principe 4).
  - Transactions `BEGIN IMMEDIATE` dans `attach` et `refresh_series` (review Ponytail) : une transaction SQLite qui lit avant d'écrire échoue aussitôt (« database is locked ») si un autre fichier écrit en parallèle ; test de 16 rattachements simultanés, qui échoue sans le correctif.
  - Le branchement dans un parcours de bibliothèque (parcours, `scan`, `attach`, `remove_missing`, `refresh_series`) arrive avec le job de scan (lot 2.6) ; la CLI `shiori scan` reste hors bibliothèque.
- [x] **2.3** `feat/server-skeleton` : cible lib du crate `shiori`, commande `serve`, état, erreurs JSON, `TraceLayer`, arrêt propre ; routes `health`, bibliothèques, séries, tomes, liste des pages.
  - `shiori serve [--listen 127.0.0.1:7878] [--db FICHIER]` ; les options de cache, `--jobs` et `--scan-log-days` arriveront avec les lots qui s'en servent (2.5, 2.6). Ouverture de la base partagée avec `scan` (`open_db` dans la cible lib).
  - Lectures dans `shiori-db::browse` : séries triées par nom (tri naturel, à la place du `sort_name` retiré au lot 2.2), tomes dans l'ordre `compare_in_series`, une seule entrée par tome si le même fichier est présent deux fois dans la série, couverture = page 0 du premier tome.
  - Erreurs `{ code, message }` partout, y compris route inconnue (`not_found`), mauvaise méthode (`method_not_allowed`), ID ou corps invalide (`invalid_id`, `invalid_body`) ; une panne de la base donne `internal` sans détail (journalisé côté serveur).
  - Sécurité (review Ponytail) : en écoute locale, toute requête dont le `Host` n'est pas `localhost`, `127.0.0.1` ou `[::1]` est refusée (`403 invalid_host`), parade au DNS rebinding (une page web malveillante qui ferait pointer son domaine vers 127.0.0.1 pour piloter l'API depuis le navigateur). En écoute réseau (`--listen 0.0.0.0:…`), aucune protection tant qu'il n'y a pas d'authentification. Racine de bibliothèque relative refusée (elle dépendrait du dossier de lancement).
  - Pas de pagination des séries pour l'instant (`shortcut:` dans le code). `matchit` reste en 0.8.4 : figé par axum (`=0.8.4`).
  - Tests HTTP sur de vraies fixtures (routeur en mémoire), plus un vrai serveur TCP arrêté proprement ; essai du binaire : `curl` sur `health` et `libraries`, arrêt sur Ctrl-C.
- [x] **2.4** `feat/server-pages` : page originale par `archive_index` avec vérifications, en-têtes immuables, codes d'erreur ; **test HTTP de l'invariant d'ordre de lecture** ; mesure d'ouverture de l'annuaire sur le webtoon réel.
  - `GET /api/pages/{id}/image` : emplacement encore identique au scan (taille, date ; une autre copie du même tome sert si la première a disparu), entrée à l'`archive_index` vérifiée (nom brut, CRC), CRC revérifié à la lecture. `Cache-Control: public, max-age=31536000, immutable`, `ETag` fort `"<page_id>"`, `304` sur `If-None-Match` (liste, `W/`, `*`). Codes : `invalid_id`, `page_not_found`, `page_broken`, `volume_unavailable`, `volume_changed`, `page_mismatch`, `page_corrupt`.
  - `X-Content-Type-Options: nosniff` sur les images (contenu venu d'archives extérieures, review Ponytail).
  - Toute réponse d'erreur porte `Cache-Control: no-store` : un `410` mis en cache bloquerait une page redevenue lisible après rescan (même fichier, même ID).
  - **Invariant d'ordre vérifié par HTTP** sur l'archive rangée dans le désordre, le faux webtoon (300 pages) et le faux manga (JPEG compris) : la page servie à l'ordinal n est l'image de l'ordinal n (comparaison exacte pour le PNG, couleur la plus proche pour le JPEG : une tolérance fixe aurait accepté une page voisine). Test vérifié en échec quand le serveur lit une autre entrée.
  - **Mesure sur le webtoon réel** (1,96 Go, 12 760 pages, build release) : ouvrir l'annuaire central coûtait ~11 ms par requête (même image seule dans un petit tome : 1,5 ms). D'où un cache des 4 dernières archives ouvertes, indexé par (chemin, taille, date) : médiane 12,9 → 2,9 ms à la suite, 13,1 → 6,4 ms au hasard, 304 en 1,5 ms, liste des 12 760 pages (1,5 Mo de JSON) en 0,44 s, mémoire du serveur 16 Mo. Test : un fichier modifié puis rescanné n'est jamais lu avec l'ancien annuaire (vérifié en échec avec une clé sans la date).
- [x] **2.5** `feat/image-resize` : `render.rs` + `cache.rs`, largeurs fixes, AVIF + JPEG, sémaphore, plafond de pixels, éviction ; mesures sur le webtoon réel.
  - `GET /api/pages/{id}/image?w=<largeur>&f=avif|jpeg` (`avif` par défaut) : largeurs de la liste fermée seulement (`invalid_width`), `invalid_format`, `invalid_query` ; `render_failed` si le décodage échoue. Une page déjà plus étroite que demandé, ou AVIF (pas de décodeur pur Rust), est servie originale avec l'ETag de l'originale ; sinon ETag `"<page>-w<largeur>-<ext>-v<RENDER_VERSION>"`.
  - **Pour le lecteur (phase 3)** : sur `render_failed`, retomber sur l'originale (`/image` sans paramètres). Cas réel : une bande de webtoon de 2000×60 000 (≈ 360 Mio décodée) dépasse le plafond de décodage alors que l'originale se lit (review Ponytail ; le plafond protège une machine de 4 Gio).
  - Rendu : décodage plafonné à 256 Mio, Lanczos3, JPEG qualité 85, AVIF `ravif` 0.13 sans `asm` (qualité 70, vitesse 10). Au plus 4 rendus simultanés (`shortcut:` : réglé pour une machine de 4 Gio).
  - Cache : `<cache>/pages/<2 car.>/<page>-w<largeur>-v<version>.<ext>`, dossier de cache de l'OS ou `SHIORI_CACHE_DIR` / `--cache-dir`, plafond `--cache-max-mib` (défaut 2048). Écriture atomique (fichier temporaire puis renommage), date rafraîchie à chaque lecture, éviction des moins récemment lus jusqu'à 90 % du plafond (un fichier verrouillé sous Windows est retenté plus tard). Un cache impossible à écrire n'empêche pas de servir la page. Les vérifications de l'originale (fichier inchangé, etc.) s'appliquent aussi aux versions en cache.
  - **Mesures sur le webtoon réel** (pages 800×1280, release) : originale 3 ms ; JPEG 480 px 91 ms au premier rendu (83 Kio) ; **AVIF 640 px 388 ms au premier rendu** (max 1,3 s, 65 Kio contre 164 Kio pour l'originale) ; AVIF 160 px 103 ms ; tout rendu en cache 2 ms ; mémoire du serveur 108 Mo au plus fort. En pratique, un téléphone (1080 px) ou une tablette 8" reçoit l'originale de ce webtoon : le rendu sert surtout aux miniatures et aux grandes pages de manga.
  - Tests : invariant d'ordre sur les pages redimensionnées (archive en désordre, couleurs espacées pour survivre au JPEG), tailles, AVIF valide, page étroite et source AVIF servies telles quelles, cache relu (fichier de cache modifié → servi tel quel), 304, paramètres invalides, rendu en échec, cache non inscriptible. `data_dir` et `cache_dir` fusionnés en une seule fonction testée.
- [x] **2.6** `feat/scan-job` : migration `0003`, job unique avec progression et annulation, passes et journaux aussi pour la CLI, passe `interrupted` au démarrage, purge.
  - `shiori::job::Scanner` : `start` (une passe à la fois, `AlreadyRunning` sinon), `progress`, `cancel`, `wait`, `shutdown`. Cible validée : bibliothèque, ou dossier / fichier sous sa racine, sans `..` ni lien symbolique (`OutsideLibrary`, `NotFound`, `UnknownLibrary`).
  - Passe : parcours trié, `files_total` écrit avant de scanner, scan parallèle (`--jobs`), rattachement à la série, journal de chaque fichier rescanné et de chacun de ses problèmes ; à la fin, retrait des fichiers disparus (bibliothèque ou dossier) et recalcul des séries. Annulation vérifiée entre deux fichiers : la passe finit `cancelled`, rien n'est retiré. Erreur de base : `failed` avec le message.
  - Compteurs : `files_ok`, `files_warning`, `files_error` comptent tous les fichiers vus, inchangés compris ; `files_unchanged` en est un sous-ensemble.
  - Purge (`--scan-log-days`, défaut 30) au démarrage du serveur et à la fin de chaque passe ; **une passe d'où viennent les problèmes actuels d'un tome (`scan_issues`) n'est jamais purgée**, sinon ses problèmes partiraient en cascade (écart au plan, trouvé en écrivant la migration).
  - `shiori serve --jobs N --scan-log-days 30` ; au démarrage, les passes restées `running` passent à `interrupted` ; à l'arrêt (Ctrl-C), le scan en cours est annulé et sa passe close.
  - La CLI écrit `files_total`, ses compteurs et son journal ; une erreur fatale clôt sa passe `failed` avec le message. Au démarrage du serveur, seules ses propres passes (`origin = 'api'`) restées `running` passent à `interrupted` : une CLI peut scanner pendant que le serveur démarre (review Ponytail).
- [x] **2.7** `feat/admin-scans-api` : routes `/api/admin/scans…` et leurs tests.
  - `POST /api/admin/scans` `{ library_id, path?, deep?, rehash? }` → `202 { run }` ; `GET /api/admin/scans/current` (progression, ou 204) ; `POST /api/admin/scans/current/cancel` (202, `404 no_scan_running`) ; `GET /api/admin/scans?status=&library_id=&before=&limit=` (du plus récent au plus ancien) ; `GET /api/admin/scans/{id}` (options, durée, compteurs, erreur) ; `GET /api/admin/scans/{id}/logs?level=&after=&limit=` (`level` = gravité minimale). Listes : 50 par défaut, 500 au plus.
  - Codes : `scan_already_running` 409, `path_outside_library` 400, `path_not_found` 404, `not_found`, `invalid_body`, `invalid_query`, `invalid_id`, `internal`.
  - **Essai de bout en bout par HTTP** sur le webtoon réel (binaire release, bibliothèque créée par l'API) : scan en 69 s (disque froid) avec progression visible, passe `completed` (1 avertissement : `non_image_entry` pour `index.json`, `archive_order_differs`), série « Leviathan » et ses 12 760 pages consultables, page 101 servie originale puis en JPEG 480 px, rescan en 1,2 s (fichier inchangé). Limite constatée : la progression compte par fichier, un seul gros fichier reste à `done: 0` jusqu'à sa fin.
- [x] **2.8** `feat/cbr` : `RarArchive` (trait `Archive`), scan en flux, lecture directe des non solid, cache d'extraction des solid, fixtures générées, invariant d'ordre en HTTP sur les deux ; conception réalisée dans [docs/cbr.md](docs/cbr.md) §6.
  - Format reconnu au contenu (`Rar!`), pas à l'extension ; `volumes.format` = `cbz` ou `cbr`. Nouvelle méthode du trait `read_each` (une passe pour tout le tome), codes `solid_archive` (info) et `multivolume_archive` (erreur). Cache `<cache>/extract/<tome>/<index>` sous le même plafond que les rendus, jamais le dernier tome lu évincé.
  - **Essai sur deux vrais CBR** (binaire release, bibliothèque et scan par l'API) : `Toriko-v16.cbr` (RAR 4 non solid, 142 Mo, 210 pages) et `tome-67.cbr` (RAR solid créé par WinRAR, 119 Mo, 183 pages dans 10 dossiers de chapitre, rangées hors ordre naturel). Scan des deux en 4,2 s. Toriko : page en 7 à 18 ms. Tome 67 : première page 1,7 à 1,9 s disque chaud (4,8 s à froid), le temps d'extraire tout le tome (114 Mo dans le cache), puis 2 ms par page ; AVIF 640 px en 0,36 s. Serveur à 69 Mo de RAM.
  - Couverture : 97,1 % des lignes, 91,1 % des branches.
- [x] **2.9** `chore/phase2-wrap` : essai réel, mise à jour de CLAUDE.md (commandes `serve`), bilan de phase ici, `/graphify . --update`.
  - **Essai final de bout en bout** (binaire release, uniquement par l'API) : une bibliothèque avec les trois vrais fichiers (webtoon CBZ de 2 Go et 12 760 pages, Toriko CBR non solid, tome 67 CBR solid WinRAR). Scan des trois en 24,5 s (webtoon déjà en mémoire), progression visible, 3 séries. Première, dernière et page du milieu de chaque tome en 2 à 25 ms (première page du solid : 2,7 s, extraction comprise), AVIF 640 px en 0,3 à 0,6 s, JPEG 320 px en 30 à 140 ms. Rescan sans changement en 152 ms. Arrêt propre par Ctrl+C, puis redémarrage : historique conservé, tome solid relu depuis le cache en 16 ms.
  - Aide de `serve` corrigée : `--quiet` décrit aussi son effet sur `serve`, `--cache-dir` et `--cache-max-mib` mentionnent les CBR solid extraits.
  - Graphe Graphify mis à jour (`--update`) : 1 086 nœuds, 2 349 arêtes, 55 communautés, aucune arête pendante ni fusionnée. Code par AST ; `CLAUDE.md`, `ROADMAP.md` et `docs/cbr.md` réextraits sémantiquement (87 814 jetons), les nœuds de leur ancienne extraction remplacés. Nœuds centraux : `Db`, `ArchiveError`, `IssueCode`, `write_zip`, `FileReport`, `App`. Gabarit Vite (README, images) toujours écarté ; les migrations SQL restent sans nœud (grammaire SQL absente).

### Bilan de la phase 2

- Livré :
  - `shiori serve` : API JSON des bibliothèques, séries, tomes et pages, protégée contre le DNS rebinding en écoute locale. Pages servies par leur ID, originales ou redimensionnées (AVIF ou JPEG, 9 largeurs), URL immuables et ETag fort, erreurs explicites `{ code, message }` sans cache.
  - Parseur de noms de tomes (110 cas, `PARSER_VERSION` figée), bibliothèques et séries (un dossier par série, fichiers de la racine regroupés).
  - Job de scan (une passe à la fois, annulation, journaux, conservation configurable) et API d'administration des scans.
  - CBR via `rars` (pur Rust) : non solid lu par index, solid extrait une fois dans le cache, sous un seul plafond partagé avec les rendus.
- Validé sur de vrais fichiers : webtoon de 2 Go (12 760 pages, archive en désordre), bibliothèque réelle de 450 fichiers pour le parseur, CBR RAR 4 non solid et CBR solid créé par WinRAR.
- Qualité : 166 tests, CI Linux / Windows / macOS verte, couverture 97,1 % des lignes et 91,1 % des branches. 9 PR (#11 à #19), chacune passée en review Ponytail.
- Reste ouvert pour la suite :
  - la progression d'un scan compte par fichier : un seul très gros fichier reste à `done: 0` jusqu'à sa fin ;
  - la première page d'un tome solid attend son extraction complète (environ 2 s pour 120 Mo), une seule extraction à la fois (`shortcut:` dans le code) ;
  - le cache n'efface un tome extrait ou un rendu que par rotation (plafond atteint, le moins récemment lu part) : un tome supprimé ou modifié y reste jusque-là ;
  - pas de décodeur AVIF pur Rust : une page AVIF est servie en originale, jamais redimensionnée ;
  - chiffres pleine chasse (`第１巻`) non reconnus par le parseur ; CBR multivolume refusé.

### Questions de la phase 2 (résolues le 2026-11-11)

- **Q1 CBR** ✅ : géré en phase 2 (lot 2.8) avec `rars` (pur Rust, mesuré sur un vrai CBR, aussi rapide que libarchive sur un cœur et plus rapide en parallèle) ; RAR non solid lu directement par index, RAR solid servi depuis un cache d'extraction. Mesures, provenance et conception : [docs/cbr.md](docs/cbr.md).
- **Q6 Formats** ✅ (option b) : AVIF (`ravif`) pour les pages, JPEG pour les miniatures et couvertures, tout en Rust pur ; le client choisit `f` dans l'URL, sans bascule automatique. Principe 7 amendé dans CLAUDE.md. Pas de WebP (pas d'encodeur avec perte pur Rust, et limite de 16 383 px).
- **Q9 Rangement** ✅ : un dossier par série sous la racine de la bibliothèque ; le parseur sert de repli pour les fichiers posés à la racine.
- **Q7 Cache** ✅ : 2 Gio par défaut (images redimensionnées et tomes RAR solid extraits), éviction des plus anciens.
- **Q8 Largeurs** ✅ : `160, 320, 480, 640, 800, 1080, 1280, 1600, 2048`.
- **Q10 Réseau** ✅ : pas d'authentification en phase 2, écoute sur `127.0.0.1` par défaut, ouverture au réseau par `--listen`.

## Phase 3 : lecteur web (plan validé le 2026-11-12)

### Objectif

Ouvrir un tome dans le navigateur, servi par le même binaire, et le lire jusqu'au bout sans jamais voir une mauvaise page : manga de droite à gauche, BD de gauche à droite, webtoon en défilement vertical continu, page simple ou double, reprise exacte là où on s'était arrêté, sur n'importe quel appareil.

Le lecteur ne décide rien lui-même : il reçoit la liste ordonnée des pages (`ordinal`, dimensions, type, statut) et affiche chaque page par son ID (invariant de lecture, principe 3). Une page `broken` ou une erreur de lecture s'affiche **à sa place** comme une erreur explicite avec son code, jamais remplacée par une autre image (principe 4).

Hors périmètre :
- la bibliothèque façon Plex (grille de couvertures, « Continuer la lecture », recherche, filtres) et le menu d'administration : phase 4 ;
- les métadonnées : phase 5 ;
- l'authentification et les comptes : « Plus tard » ;
- la PWA finalisée (installation, hors ligne) : phase 6. La phase 3 pose seulement les zones sûres et le plein écran.

### Mesures qui orientent le plan

Faites le 2026-11-12, sur les trois vrais fichiers :

| Fichier | Pages | Largeur (min / médiane / max) | Hauteur max | Pages plus larges que hautes |
|---|---|---|---|---|
| Webtoon CBZ | 12 760 | 480 / 800 / 800 | 1 280 | 70 (fins de bandes, jusqu'à 1 px de haut) |
| Major T67 (CBR solid) | 183 | 814 / 1 005 / 2 011 | 1 602 | 2 doubles pages |
| Toriko T16 (CBR) | 210 | 1 200 | 1 883 | 0 |

- **Le webtoon réel est déjà découpé en bandes de 800×1280** par l'outil de téléchargement. Découper les très hautes images côté serveur (prévu dans le README) n'a pas de cas réel aujourd'hui : reporté, avec repli sur l'originale si un rendu échoue (`render_failed`, lot 2.5).
- **12 760 pages** : le mode vertical doit être virtualisé (seules les pages proches de l'écran existent dans le DOM), avec la place de chaque page réservée d'après ses dimensions du scan, donc aucun saut de mise en page.
- Une page « plus large que haute » n'est une double page qu'en mode paginé : en vertical, une fin de bande de 800×1 est une bande comme une autre.

### Découpage

```
backend/crates/shiori-db   + migration 0004_reading.sql : progression par tome, préférences de lecture
backend/crates/shiori      + web.rs : front embarqué (rust-embed), repli SPA, en-têtes de cache
                           + routes de progression et de préférences
web/                       Tailwind 4 + shadcn/ui, React Router, TanStack Query, Vitest
  src/api/                 client typé de l'API (types alignés sur les réponses JSON)
  src/reader/              lecteur : calcul des vues (pures, testées), modes paginé et vertical
  src/browse/              navigation minimale provisoire : bibliothèques → séries → tomes
```

### Dépendances ajoutées (dernières versions au 2026-11-12)

| Où | Paquet | Rôle |
|---|---|---|
| backend | `rust-embed` 8.14 (pur Rust, MIT) | fichiers de `web/dist` dans le binaire en release, lus sur le disque en debug |
| web | `tailwindcss` 4.3 + `@tailwindcss/vite` | styles (direction UI de CLAUDE.md) |
| web | `shadcn` 4.23 (composants copiés dans le projet) | boutons, curseur, menus de la barre d'outils |
| web | `react-router` 8.5 | routes `/`, `/series/:id`, `/read/:volumeId` |
| web | `@tanstack/react-query` 5.107 | cache des réponses de l'API, préchargement |
| web | `@tanstack/react-virtual` 3.14 | liste virtualisée du mode vertical |
| web (dev) | `vitest` 5.1, `@vitest/coverage-v8`, `@testing-library/react`, `@testing-library/user-event`, `jsdom` | tests et couverture ≥ 80 % lignes et branches |
| web (dev) | `@playwright/test` 1.65 | bout en bout dans un vrai navigateur, contre `shiori serve` (Q16) |

### Base : migration `0004_reading.sql`

```sql
-- Progression : une ligne par tome lu. Clé = identité du tome (hash du fichier, principe 8) :
-- renommer ou déplacer le fichier ne perd rien.
CREATE TABLE reading_progress (
  volume_id   TEXT PRIMARY KEY REFERENCES volumes(id) ON DELETE CASCADE,
  page_id     TEXT NOT NULL,     -- dernière page vue (identité stable, jamais un index brut)
  ordinal     INTEGER NOT NULL,  -- sa position, pour l'affichage « 42 / 183 »
  finished_at TEXT,              -- dernière page atteinte
  updated_at  TEXT NOT NULL
);
-- Préférences de lecture par série ; NULL = défaut de la bibliothèque (Q12).
ALTER TABLE series ADD COLUMN reading_mode TEXT CHECK (reading_mode IN ('rtl','ltr','vertical'));
ALTER TABLE series ADD COLUMN page_layout  TEXT CHECK (page_layout IN ('single','double','auto'));
-- Q12 : type de bibliothèque, qui donne le mode par défaut de ses séries.
ALTER TABLE libraries ADD COLUMN kind TEXT NOT NULL DEFAULT 'manga' CHECK (kind IN ('manga','comics','webtoon'));
```

- Un tome dont le fichier change reçoit une nouvelle identité : sa progression ne s'applique pas à un contenu différent. Elle n'est pas transférée en phase 3 ; à rediscuter si le cas se présente (fichier retouché par un outil).
- Une page de progression qui n'existe plus dans le tome (impossible tant que l'identité du tome est le hash du fichier) est refusée à l'écriture : `page_id` doit appartenir au tome.

### Routes ajoutées

| Méthode | Route | Rôle |
|---|---|---|
| GET | `/api/volumes/{id}/progress` | `{ page_id, ordinal, page_count, finished }` ou `204` si jamais ouvert |
| PUT | `/api/volumes/{id}/progress` | `{ page_id }` → enregistre ; `finished` calculé par le serveur (dernier ordinal) ; `422 page_not_in_volume` |
| DELETE | `/api/volumes/{id}/progress` | « marquer comme non lu » |
| PATCH | `/api/series/{id}/settings` | `{ reading_mode?, page_layout? }` (`null` = revenir au défaut) |
| — | `GET /api/series/{id}` et `GET /api/volumes/{id}` | gagnent `progress` par tome et les réglages effectifs (`reading_mode`, `page_layout`) |
| — | `POST /api/libraries` | gagne `kind` (défaut `manga`) |
| GET | `/`, `/series/…`, `/read/…` | front embarqué ; toute route inconnue hors `/api` renvoie `index.html` (repli SPA) |

En-têtes du front : fichiers de `assets/` (noms avec hash de Vite) en `Cache-Control: public, max-age=31536000, immutable` ; `index.html` en `no-cache`, pour qu'une mise à jour du binaire soit vue tout de suite. Le binaire compile même si `web/dist` est absent (dev du backend seul) et sert alors une page qui explique de lancer `npm run build`.

### Lecteur

- **Route** `/read/:volumeId`, plein écran, barre d'outils masquée ; un tap au centre l'affiche. Bouton « Retour » explicite (principe PWA de CLAUDE.md).
- **Modes** : `rtl` (manga), `ltr` (BD), `vertical` (webtoon). Mode et disposition se changent dans la barre d'outils et sont retenus pour la série (`PATCH /settings`).
- **Disposition en mode paginé** : `single`, `double`, `auto` (double en paysage, simple en portrait). En double : la couverture (ordinal 0) seule, une page plus large que haute toujours seule, les autres par deux dans le sens de lecture. Un bouton « décaler d'une page » corrige un tome dont les doubles pages tombent mal.
- **Doubles pages en mode simple** (Q14) : **coupées par défaut en deux moitiés successives** (droite puis gauche en `rtl`, gauche puis droite en `ltr`), pour ne pas avoir à tourner la tablette ; affichage en entier, réduit à l'écran, activable dans la barre d'outils. La coupe se fait côté client sur la même image (même URL immuable, rien de nouveau côté serveur) ; chaque moitié compte comme une vue, la progression enregistre la page.
- **Calcul des vues** dans des fonctions pures (`views(pages, mode, layout, orientation, offset)` → suite de vues de 1 ou 2 pages) : c'est là que vit toute la logique, testée à part, table de cas comme le parseur de noms.
- **Images** : largeur demandée = la plus petite de la liste fixe ≥ largeur affichée × `devicePixelRatio` (une page AVIF ou plus étroite que demandé : l'originale, comme le serveur le fait déjà). Format AVIF ; sur `render_failed`, repli sur l'originale. Préchargement des 2 vues suivantes et de la précédente. Place réservée par `aspect-ratio` d'après `width`/`height` du scan.
- **Erreurs à leur place** : une page `broken` (`422 page_broken`), un fichier changé (`409 volume_changed`), un tome introuvable (`410 volume_unavailable`)… s'affichent comme une carte d'erreur à l'emplacement de la page, avec le code et un message en français. Le lecteur ne saute jamais une page et ne répète jamais la précédente.
- **Navigation** : zones de tap gauche / droite (selon le sens de lecture), balayage, clavier (flèches, espace, Page précédente / suivante, Début / Fin), curseur de pages dans la barre d'outils (« 42 / 183 »). Fin de tome : carte « Tome suivant » (ordre de `GET /api/series/{id}`, déjà trié par le serveur).
- **Mode vertical** : liste virtualisée, largeur ajustée à l'écran (plafonnée à 800 px de contenu sur un grand écran), position de lecture = première page visible.
- **Progression** : enregistrée environ 1 s après le dernier changement de page et quand l'onglet passe en arrière-plan (`fetch` avec `keepalive`) ; à l'ouverture, reprise sur la page enregistrée (lien direct possible : `/read/:volumeId?page=<page_id>`).
- **Zoom** (Q15) : double tap pour zoomer ×2 et déplacement au doigt en mode paginé ; pincement natif du navigateur en vertical.

### Navigation provisoire (en attendant la phase 4)

Trois écrans simples pour atteindre le lecteur : bibliothèques, séries d'une bibliothèque, tomes d'une série (avec la progression de chacun). Thème sombre et couleurs de CLAUDE.md, mais pas de grille de couvertures ni de sidebar : la phase 4 les remplacera. Créer une bibliothèque et lancer un scan restent à faire par l'API ou la CLI.

### Tests

- **Rust** : migration 0004, routes de progression et de réglages (cas d'erreur compris), front embarqué (index, assets immuables, repli SPA, `/api` inconnu reste un 404 JSON, `web/dist` absent).
- **Vitest** : calcul des vues (table de cas : simple, double, couverture seule, double page seule, décalage, `rtl`/`ltr`, orientation), choix de la largeur, client de l'API, composants du lecteur (carte d'erreur à la place d'une page cassée, jamais une autre image ; navigation clavier et tap ; enregistrement de la progression).
- **Bout en bout (Playwright)** contre `shiori serve` sur les fixtures de `shiori-testkit` : l'invariant de lecture dans un vrai navigateur (la vue n affiche la couleur de l'ordinal n, en CBZ et en CBR solid), une page cassée affichée en erreur à sa place, reprise après rechargement. Captures en 390×844, 800×1280, 1280×800 et 1440×900 (CLAUDE.md).
- Couverture front : seuils Vitest à 80 % lignes et branches, ajoutés au job CI `web`.

### Lots (une branche + une PR chacun)

- [x] **3.1** `feat/web-foundation` : retrait du gabarit Vite, Tailwind 4, shadcn/ui, thème sombre de CLAUDE.md, React Router, TanStack Query, client typé de l'API, Vitest avec seuils de couverture dans la CI.
  - shadcn/ui posé à la main (`components.json`, `cn()` avec `clsx` et `tailwind-merge`) : `shadcn init` reste bloqué sur une question interactive malgré `-y`. Les composants viendront avec `shadcn add` quand un écran en aura besoin.
  - Thème : variables au format shadcn (`#121212`, `#1E1E1E`, `#2A2A2A`, ambre `#E5A00D` exact, différent de l'`amber-500` de Tailwind), utilitaires `pt-safe`/`pb-safe`/`pl-safe`/`pr-safe`, `viewport-fit=cover` (sans lui les zones sûres valent 0).
  - Client de l'API : erreurs `ApiError { status, code, message }` avec le code stable du serveur, `network` si le serveur est injoignable ; une erreur 4xx n'est jamais réessayée par TanStack Query. `pageImageUrl` ne connaît que la liste fixe des largeurs.
  - `npm run dev` relaie `/api` vers `shiori serve` sur `127.0.0.1:7878`. Vérifié dans Chromium contre le vrai serveur.
  - Job CI `web` : `npm run coverage` (seuils de 80 % lignes et branches) avant le build. 15 tests, couverture 100 %.
  - **Reprise UI** (PR à part, après coup) : skills UI/UX Pro Max, MCP shadcn et captures Playwright aux quatre tailles, qui n'avaient pas été utilisés au lot 3.1.
    - Contrastes WCAG calculés : texte 17:1, secondaire ≥ 5,6:1, ambre 8,3:1. Corrigés : texte d'erreur `#F87171` (le rouge d'origine faisait 4,4:1 sur une carte), fond des boutons destructifs `#B91C1C` (6,5:1 avec le blanc, au lieu de 2,8:1), bordure des champs `#71717A` (≥ 3,4:1, règle des éléments non textuels).
    - L'application est toujours sombre : les variantes `dark:` des composants shadcn suivent la classe `dark` de `<html>`, pas le thème de l'OS (vérifié avec un OS émulé en clair).
    - Focus clavier visible partout : contour ambre de 2 px décalé de 2 px (8,3:1). L'anneau ambre à 50 % du Button shadcn, trop pâle autour d'un bouton ambre, est retiré. Mouvements réduits respectés (`prefers-reduced-motion`).
    - Button ajouté par le MCP shadcn, tailles relevées pour la tablette (44 px par défaut). La commande générée importait `cn` depuis un paquet npm du même nom (installé sans raison) au lieu de `@/lib/utils`, et oubliait `class-variance-authority` : corrigé. **À chaque `shadcn add` : vérifier les imports et les dépendances** (liste de contrôle du MCP).
    - Police : pile système de Tailwind, sans police web (hors ligne, rapide, rien chargé depuis un service tiers).
- [x] **3.2** `feat/embed-web` : `rust-embed`, repli SPA, en-têtes de cache, binaire qui compile sans `web/dist`.
  - `web.rs`, générique sur le dossier intégré (testé avec un petit dossier de fixtures) : le routeur sert l'API sous `/api` (route inconnue = 404 JSON, `/api` compris) et le front partout ailleurs, derrière la même protection DNS rebinding.
  - Repli SPA seulement pour un chemin sans extension (`/read/…`) : un fichier introuvable (vieux `assets/index-abc.js` après une mise à jour) reste un vrai 404, jamais `index.html` servi à la place d'un script. Autre méthode que GET/HEAD hors `/api` : 405 JSON.
  - En-têtes : `assets/` (noms avec hash de Vite) immuables ; `index.html` et le reste en `no-cache` avec ETag (sha256) et `304`. `nosniff` partout.
  - Sans `web/dist` (compilation du backend seul, CI) : `503` et une page qui explique `npm run build`. En debug les fichiers sont relus sur le disque à chaque requête : reconstruire le front suffit, sans recompiler.
  - **Vérifié** : binaire release construit, puis `web/dist` déplacé : le front est toujours servi (intégré), `/read/abc` donne `index.html`, l'asset haché est immuable, un vieil asset fait 404, `If-None-Match` donne 304.
  - Pas de binaire de release dans la CI aujourd'hui : construire le front avant `cargo build --release` sera une étape du job de release (phase 6, distribution).
- [x] **3.3** `feat/reading-progress` : migration 0004, `kind` des bibliothèques, routes de progression et de réglages, progression dans les réponses des séries et des tomes.
  - `shiori-db::reading` : `LibraryKind` (manga → `rtl`, comics → `ltr`, webtoon → `vertical`), `ReadingMode`, `PageLayout` (défaut `auto`), `ReadingSettings` (valeurs appliquées + ce que la série impose, `null` = défaut de la bibliothèque), `Progress`.
  - Progression : `PUT` refuse une page d'un autre tome (`422 page_not_in_volume`) ; atteindre la dernière page marque le tome lu, et il le reste si l'on revient en arrière (seul `DELETE` l'efface) ; `updated_at` servira à « Continuer la lecture » (phase 4).
  - `PATCH /api/series/{id}/settings` : champ absent = inchangé, `null` = retour au défaut, champ inconnu refusé (`400 invalid_body`).
  - Réglages d'un tome (`GET /api/volumes/{id}`) : ceux de sa première série par chemin ; un tome rattaché à aucune série prend le défaut d'une bibliothèque de manga.
  - `GET /api/series/{id}` : `settings` et `progress` par tome (absent si jamais ouvert) ; `GET /api/libraries` et `POST` : `kind` (`manga` si absent). Les bibliothèques existantes deviennent `manga`.
  - Types TypeScript du front alignés sur ces réponses.
  - Couverture : 97,3 % des lignes, 91,3 % des branches (`reading.rs` à 100 %).
- [ ] **3.4** `feat/browse-minimal` : les trois écrans provisoires.
- [ ] **3.5** `feat/reader-paged` : lecteur paginé simple (`rtl`/`ltr`), choix de la largeur, préchargement, repli sur l'originale, cartes d'erreur, navigation (tap, balayage, clavier, curseur).
- [ ] **3.6** `feat/reader-double` : disposition double et `auto`, couverture et doubles pages seules, décalage, coupe des doubles pages en mode simple.
- [ ] **3.7** `feat/reader-vertical` : mode webtoon virtualisé, essai sur le webtoon réel de 12 760 pages.
- [ ] **3.8** `feat/reader-progress` : reprise, enregistrement, réglages retenus par série, tome suivant, zoom.
- [ ] **3.9** `test/reader-e2e` : Playwright dans la CI, invariant de lecture dans le navigateur, captures aux quatre tailles et corrections d'alignement.
- [ ] **3.10** `chore/phase3-wrap` : essai réel sur tablette (ou sa taille), README à jour, bilan de phase, `/graphify . --update`.

### Questions de la phase 3 (résolues le 2026-11-12)

- **Q11 Progression** ✅ : **côté serveur**, en base (un seul utilisateur implicite tant qu'il n'y a pas de comptes), pour reprendre la même page sur la tablette et sur le PC.
- **Q12 Mode par défaut** ✅ : **type de bibliothèque** choisi à sa création (`manga` → droite à gauche, `comics` → gauche à droite, `webtoon` → vertical) ; un réglage par série l'emporte.
- **Q14 Doubles pages en mode simple** ✅ : **coupées en deux moitiés successives par défaut** (« c'est pénible de tourner la tablette ») ; affichage en entier activable dans la barre d'outils.

Propositions retenues :

- **Q13 Disposition par défaut** : `auto` (double en paysage, simple en portrait), couverture seule.
- **Q15 Zoom** : double tap ×2 et déplacement en mode paginé, pincement natif en vertical, dans le lot 3.8.
- **Q16 Playwright dans la CI** : oui, job Linux seulement (navigateur Chromium, environ 1 à 2 min de plus par PR).
- **Q17 Découpage serveur des très hautes images** : reporté, aucun cas réel (voir les mesures) ; repli sur l'originale en attendant.
- **Q18 Breakpoint tablette 8" en portrait** (CLAUDE.md) : sans objet en phase 3 (le lecteur en est exclu, la navigation provisoire est une simple liste) ; à valider en phase 4.

## Fonctionnalités ajoutées en cours de route

### Menu d'administration des scans (demandé le 2026-11-11)

Objectif : piloter les scans depuis l'application et consulter leurs journaux, sans terminal.

- **Phase 2 (API, lots 2.6 et 2.7)** : lancer un scan (une bibliothèque, un dossier, un tome ; options `deep` et `rehash`), voir le scan en cours (progression, fichier courant), l'annuler ; historique des passes (`scan_runs` : début, fin, options, durée, compteurs ok / avertissement / erreur / inchangés) ; journal détaillé d'une passe (fichiers scannés, problèmes, erreurs fatales). À prévoir : une table de journal par passe (horodatage, niveau, fichier, message) et une durée de conservation configurable.
- **Phase 4 (UI)** : page « Administration » → « Scans » : bouton « Scanner maintenant », passe en cours avec progression, historique filtrable, détail d'une passe avec ses journaux et un lien vers chaque tome en erreur. Accès réservé aux administrateurs quand les rôles arriveront (phase « Plus tard »).

### Documentation (demandée le 2026-11-11)

À produire en fin de projet (phase 6), à partir de CLAUDE.md, ROADMAP.md, `docs/` et du graphe Graphify (`graphify query`, `explain`, `path`, wiki) :

- **Documentation technique** : architecture (crates, flux scan → base → API → lecteur), schéma SQLite, routes API, identités et invariants, choix de dépendances et leurs raisons.
- **Documentation d'installation**, expliquée simplement : Docker, binaire Linux/Windows, premier lancement, ajout d'une bibliothèque, mise à jour, sauvegarde de la base.
- **Documentation fonctionnelle** : bibliothèques et séries, rangement conseillé des fichiers, lecture, scans et leurs journaux, signification de chaque code de diagnostic.

### Invariant de lecture à vérifier en phase 2

L'ordre de lecture ne dépend jamais de l'ordre dans l'archive : le lecteur reçoit la liste ordonnée des `page_id` (par `ordinal`), et le serveur lit chaque page par l'`archive_index` enregistré pour cet ID. Test d'intégration prévu au lot 2.4 : sur une archive rangée dans le désordre, la page n servie par HTTP est l'image attendue à l'ordinal n (même contrôle par couleur que `every_page_is_the_right_image`).

## Journal des décisions

- 2026-11-11 : dépôt cloné depuis `befa160/shiori` (licence MIT). `graphify-out/` ignoré par git. Hooks Graphify dans `.claude/settings.json` versionné. Playwright MCP en Chromium headless (scope projet).
- 2026-11-11 : travail par branches et PR pour tout le projet.
- 2026-11-11 : plan de la phase 1 validé, propositions Q1 à Q5 retenues.
- 2026-11-11 : fixtures AVIF : une image 8×12 encodée une fois avec `ravif` et intégrée en constante (287 octets), pour ne pas tirer un encodeur AV1 dans les dépendances de test. `shiori-testkit` est exclu de la mesure de couverture (code de test). Dépendances compilées en `opt-level = 3` même en debug pour que les fixtures se génèrent vite.
- 2026-11-11 : toujours les dernières versions stables (Rust stable, Node 26 via `web/.node-version`, dernières crates/paquets/actions), pas de support des versions anciennes. Dependabot (cargo, npm, GitHub Actions) en phase 6.
- 2026-11-11 : couverture minimale 80 % lignes ET branches, imposée par le job CI `coverage` (nightly + `cargo-llvm-cov --branch`, fichiers de tests exclus de la mesure). Tests unitaires déplacés dans des fichiers `tests.rs` voisins.
- 2026-11-11 : `zip` décode en CP437 un nom UTF-8 invalide (pas d'erreur) ; les octets bruts restent la seule base des IDs.
- 2026-11-11 : formule des `page_id` figée par un test d'instantané (`id::tests::page_id_is_frozen`). Ordre naturel : à valeur égale, `9` précède `09` (comportement d'`alphanumeric-sort`, déterministe).
- 2026-11-11 : note CBR ([docs/cbr.md](docs/cbr.md)) et plan de la phase 2 proposés, en attente de validation (Q1, Q6, Q9 bloquantes).
- 2026-11-11 : plan de la phase 2 validé : CBR en phase 2 avec `rars` (lecture directe des RAR non solid, cache d'extraction des solid), images AVIF + JPEG en Rust pur, un dossier par série, défauts Q7, Q8, Q10 retenus.
- 2026-11-12 : CBR solid servi depuis un cache d'extraction en rotation (plafond partagé avec les rendus, le moins récemment lu part en premier, jamais le dernier tome lu), validé sur un vrai CBR solid WinRAR ; phase 2 terminée.
- 2026-11-12 : plan de la phase 3 proposé (lecteur web, front embarqué, progression), en attente de validation (Q11, Q12, Q14 bloquantes).
- 2026-11-12 : plan de la phase 3 validé : progression côté serveur (Q11), type de bibliothèque qui donne le sens de lecture par défaut (Q12), doubles pages coupées en deux moitiés par défaut en mode simple (Q14) ; défauts Q13, Q15 à Q18 retenus.
