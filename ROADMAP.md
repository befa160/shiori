# Roadmap Shiori

Document vivant : il est mis à jour à chaque PR qui fait avancer une phase (cases cochées, décisions, questions résolues).
Légende : ✅ fait · 🚧 en cours · ⏳ à faire · ❓ décision attendue.

| Phase | Contenu | État |
|---|---|---|
| 0 | Outillage (Rust, structure, Ponytail, Graphify, UI/UX Pro Max, MCP shadcn + Playwright) | ✅ |
| 1 | CLI de scan d'intégrité + schéma SQLite + CI linux/windows, puis graphe Graphify | ✅ |
| 2 | API axum + streaming/redimensionnement des images + parseur de noms + job de scan + API d'administration des scans + CBR | ✅ |
| 3 | Lecteur web (sens de lecture, webtoon, doubles pages) + front embarqué + progression de lecture | ✅ |
| 4 | Bibliothèque façon Plex + statuts/filtres + menu d'administration (scans, journaux) + DESIGN_SYSTEM.md | ✅ |
| 5 | Métadonnées web + édition manuelle | ✅ |
| 6 | Distribution : image Docker, PWA finalisée, binaires Linux/Windows en release, Dependabot, documentation (technique, installation, fonctionnelle), thèmes de l'interface (sombre, clair, OLED, sépia) | 🚧 |

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
| `metadata_invalid` | warning | `ComicInfo.xml` ou `index.json` à la racine illisible (XML/JSON invalide, encodage, plus de 4 Mio) : ignoré, les pages restent lues (lot 5.2) |
| `ignored_entry` | info | entrées connues et sans risque : `ComicInfo.xml` hors de la racine, `__MACOSX/`, `.DS_Store`, `Thumbs.db` (à la racine, `ComicInfo.xml` et `index.json` sont lus : lot 5.2) |

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
- Identité des séries : (bibliothèque, dossier). Renommer le dossier d'une série la fait reprendre par le nouveau dossier, avec ses réglages et ses métadonnées (lot 5.1).

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

## Phase 3 : lecteur web (plan validé le 2026-11-12, terminée le 2026-11-12)

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
- [x] **3.4** `feat/browse-minimal` : les trois écrans provisoires.
  - `/` bibliothèques (type, nombre de séries et de tomes), `/libraries/:id` séries, `/series/:id` tomes dans l'ordre du serveur : libellé tiré du parseur (« Tome 3 », « Tomes 1–3 », « Chapitre 12,5 », « Spécial », sinon le nom du fichier), nombre de pages, badge « Erreur » ou « Avertissement » (le mot porte le sens, pas seulement la couleur), « Lu », ou « 45 / 183 » avec la barre de progression ambre de 1 px de haut en bas de la carte.
  - Conception avec UI/UX Pro Max : bouton « Retour » explicite vers l'écran parent (l'historique du navigateur reste intact), lignes entièrement cliquables de 56 px au moins espacées de 8 px, titres tronqués sur deux lignes (titre complet au survol), état vide qui dit quoi faire, chargement par squelettes annoncé aux lecteurs d'écran (`aria-busy`), erreur avec le message du serveur et « Réessayer », titre d'onglet par écran.
  - Composants par le MCP shadcn : badge, squelette, barre de progression (accessible, `role="progressbar"`) ; le badge destructif reprend le fond foncé du bouton. Le bug du registre (`import { cn } from "cn"`) est revenu : corrigé, et une règle oxlint (`no-restricted-imports`) fait désormais échouer le lint s'il réapparaît. Icônes `lucide-react`.
  - Ligne de tome pas encore cliquable (`shortcut:` dans le code) : elle deviendra un lien vers le lecteur au lot 3.5.
  - Liste de contrôle finale UI/UX Pro Max : retour visuel au toucher ajouté (`active:` sur les lignes et les boutons), le survol n'existant pas sur tablette.
  - Captures Playwright aux quatre tailles, OS émulé en clair, sur une vraie bibliothèque (CBR réels, titre très long, tome abîmé, tome lu, tome en cours) : aucun défaut d'alignement, focus clavier visible sur les lignes.
  - Identifiant invalide dans l'adresse (`/series/abc`, `/libraries/0`) : « Page introuvable » sans appeler l'API (avant : message technique du serveur, en anglais). Pendant « Réessayer », la requête repasse en chargement et les squelettes le montrent.
  - 35 tests Vitest, couverture 100 % des lignes, 97 % des branches.
- [x] **3.4b** `feat/i18n` (ajouté le 2026-11-12, avant le lecteur pour qu'il naisse bilingue) : interface en français et en anglais.
  - Décisions (Q19 à Q21, ci-dessous) : français + anglais ; langue **enregistrée côté serveur** et choisie par un sélecteur, globale tant qu'il n'y a pas de comptes, par compte ensuite ; **l'interface traduit** d'après les codes stables de l'API, serveur et CLI restent en français.
  - Serveur : migration `0005_settings.sql` (une ligne `app_settings`, `language` à `NULL` tant que personne n'a choisi), `GET` / `PUT /api/settings` ; `language` est obligatoire dans le `PUT` (un corps vide ne remet rien à zéro, il faut écrire `null`).
  - Front : i18next + react-i18next, catalogues `fr.ts` (source) et `en.ts` typé sur le français (clé manquante = erreur de types), clés typées dans `t()`. Pluriels par `Intl.PluralRules` (« 0 tome » en français, « 0 volumes » en anglais), nombres par `Intl.NumberFormat` (« 12 760 » / « 12,760 »). Sans choix enregistré : langue du navigateur parmi fr / en, sinon anglais. `<html lang>` suit la langue.
  - Erreurs : `errorText` traduit le code (`volume_changed`, `network`…), précise ce qui est introuvable selon l'écran, et garde le message du serveur pour un code inconnu.
  - Sélecteur de langue (Select shadcn par le MCP) dans le cadre commun : chaque langue écrite dans sa propre langue, menu sous le sélecteur, entrées de 44 px, focus global. Le choix change la langue tout de suite et l'enregistre ; un échec d'enregistrement remet la langue d'avant.
  - Défauts trouvés par les captures Playwright et corrigés : le thème ne définissait pas la couleur `popover`, le menu du Select s'ouvrait transparent par-dessus le sélecteur. Un test vérifie désormais que chaque couleur utilisée par un composant est définie dans le thème (prouvé en échec sans `popover`). Le bug `cn` du registre shadcn est revenu avec le Select, bloqué par la règle oxlint.
  - Captures aux quatre tailles en anglais (libellés de longueurs différentes), choix gardé après rechargement (serveur à `fr`, navigateur en anglais).
  - **Système de notifications** (demandé après la review) : Sonner, la solution recommandée par shadcn (ajouté par le MCP), thème sombre fixe (sans `next-themes`), fermeture au bout de 5 s, au-dessus de la zone sûre du bas sur mobile, monté hors des routes pour que le lecteur s'en serve aussi. Premier usage : un échec d'enregistrement de la langue n'est plus silencieux (« Langue non enregistrée, réessayez plus tard » et la cause). Le test a trouvé un vrai défaut : la notification partait dans la langue qui venait d'échouer (`t` capturé au rendu) ; elle utilise désormais `i18n.t`, une fois la langue d'avant remise.
  - Le client de l'API appelle sa fonction générique `request` (elle fait aussi des `PUT`).
  - 44 tests Vitest, 100 % des lignes, 97,8 % des branches.
  - À surveiller : le script principal fait 580 ko (182 ko compressé), servi sans compression mais mis en cache pour toujours. Compression côté serveur (`tower-http`) et découpage par route à envisager avec le lecteur.
- [x] **3.5** `feat/reader-paged` : lecteur paginé simple (`rtl`/`ltr`), choix de la largeur, préchargement, repli sur l'originale, cartes d'erreur, navigation (tap, balayage, clavier, curseur).
  - `/read/:volumeId` plein écran, hors du cadre commun (il applique lui-même la langue enregistrée) ; la page courante est dans l'adresse (`?page=<id de page>`, identité stable) : un lien, un rechargement ou « retour » y revient. Les lignes de tome deviennent des liens vers le lecteur.
  - Logique en fonctions pures testées (`reader/controls.ts`) : largeur affichée ajustée à l'écran, largeur demandée = la plus petite de la liste fixe qui couvre les pixels réels (`devicePixelRatio`), originale pour une AVIF ou plutôt que d'agrandir ; touches, zones de tap et balayages selon le sens (en manga, « suivante » est à gauche, balayer vers la droite avance).
  - Place réservée par `aspect-ratio` d'après le scan, aucun saut. Préchargement des deux pages suivantes et de la précédente, à la même taille.
  - Erreur à la place de la page (principe 4) : page `broken` sans requête ; une image qui échoue est retentée en originale, puis la cause est demandée au serveur et traduite (`volume_changed`…). Carte de la forme d'une page portrait quand les dimensions sont inconnues.
  - Commandes masquées par défaut : tap au centre pour les montrer, Échap pour les masquer, **Tab** pour les montrer au clavier avec le focus sur « Retour » (invisibles, elles sont hors du parcours clavier : aucun piège). Retour vers la série, titre « Série · Tome N », boutons page précédente / suivante dans le sens de lecture, curseur de pages (Slider shadcn par le MCP, `dir="rtl"` en manga, poignée de 24 px dans une zone de 44 px), compteur formaté par `Intl`, page annoncée aux lecteurs d'écran.
  - Conception avec UI/UX Pro Max : sémantique et clavier complets ; balayage ignoré depuis les bords de l'écran (24 px), où commencent les gestes système, tap et boutons restant disponibles.
  - Webtoon lu paginé de gauche à droite en attendant le mode vertical du lot 3.7 (`shortcut:` dans le code).
  - Captures Playwright sur de vrais CBR (Toriko, le tome 67 solid, une copie abîmée) aux quatre tailles : page à la taille de l'écran (480 px demandés en 390, 800 en 800×1280), commandes, carte d'erreur, parcours clavier. Corrigé après captures : Échap montrait les commandes au lieu de les masquer, la carte d'erreur prenait tout l'écran, compteur non formaté.
  - Review Ponytail, corrigé : un pincement à deux doigts (zoom) ne tourne plus jamais la page (pointeurs comptés, geste annulé dès deux doigts) ; faire glisser le curseur ne change de page qu'au relâchement (vérifié dans Chromium : 0 image demandée pendant un glissement sur environ 200 pages, 4 au relâchement, au lieu d'un encodage AVIF par page traversée) ; clic droit ou molette ne tournent plus la page ; dans la barre de commandes, les touches vont au bouton ou au curseur qui a le focus. Trouvé par les tests : la poignée du curseur n'avait pas de nom accessible (le rôle `slider` est sur la poignée, pas sur le conteneur).
  - 75 tests Vitest, 99,3 % des lignes, 96 % des branches.
- [x] **3.6** `feat/reader-double` : disposition double et `auto`, couverture et doubles pages seules, décalage, coupe des doubles pages en mode simple.
  - Logique en fonctions pures (`reader/views.ts`, 12 cas) : la liste des pages devient une suite de **vues** (une page, deux pages, ou une moitié de double page). En double : couverture seule, page plus large que haute seule, les autres par deux, l'appariement reprenant après une double page ; « décaler d'une page » laisse aussi la deuxième page seule. `auto` : double en paysage, simple en portrait. En simple : double page coupée en deux moitiés successives par défaut (Q14), dans le sens de lecture (moitié droite d'abord en manga), entière sur demande ; une page cassée n'est jamais coupée.
  - Deux pages côte à côte, la première du côté où l'on commence à lire, chacune ajustée à la moitié de l'écran ; une moitié est coupée côté client dans la même image (largeur demandée pour deux écrans de large).
  - Adresse : la vue est retrouvée par sa première page **et sa moitié** (`?page=…&half=right`). Trouvé par le test : sans la moitié, « suivante » sur une page coupée revenait à la première moitié et le lecteur restait bloqué.
  - Panneau « Affichage » dans la barre du haut (Popover et ToggleGroup shadcn par le MCP, état actif en ambre, 44 px) : disposition Simple / Double / Auto, décalage en double, doubles pages coupées / entières en simple. Réglages locaux, retenus par série au lot 3.8.
  - Clavier : Échap ferme d'abord le panneau, puis masque les commandes et rend le focus aux pages (avant : le focus restait sur un bouton invisible et les flèches ne répondaient plus). Préchargement par vue, recalculé seulement quand les vues changent.
  - Captures Playwright sur le tome 67 (deux doubles pages de 2 011 px) : en paysage 1280 et 1440, paire en vis-à-vis (page 2 à droite en manga), double page seule ; en portrait 800 et 390, double page coupée, moitié droite puis gauche ; panneau ouvert.
  - Review Ponytail, corrigé : toucher la page pour fermer le panneau ne la tourne plus ; l'annonce aux lecteurs d'écran précise la moitié d'une double page coupée.
  - 89 tests Vitest, 99,5 % des lignes, 96,5 % des branches.
- [x] **3.7** `feat/reader-vertical` : mode webtoon virtualisé, essai sur le webtoon réel de 12 760 pages.
  - `reader/Vertical.tsx` : défilement vertical continu **du document** (défilement et clavier natifs du navigateur), `@tanstack/react-virtual` 3.14 : seules les bandes proches de l'écran existent dans la page. Colonne ajustée à l'écran, plafonnée à 800 px. Hauteur de chaque bande connue d'avance par le scan (page cassée : forme portrait 2:3), donc aucun saut ; les positions sont calculées depuis ces hauteurs (le `scrollToIndex` du virtualiseur visait 0 au premier rendu).
  - Adresse : suit la première bande visible (`?page=…`, écrite 300 ms après l'arrêt, sans réécrire l'historique à chaque pixel) ; ouverture et rechargement y reviennent ; un changement de largeur (rotation) ramène à la bande lue.
  - Clic ou tap : commandes. Curseur de pages : saut à la bande choisie au relâchement. Code partagé avec le mode paginé extrait (`chrome.tsx` : barre et curseur ; `hooks.ts` : taille de l'écran, Échap / Tab). Le mode vertical n'est plus lu en paginé (`shortcut:` du lot 3.5 retiré).
  - **Essai sur le webtoon réel** (2 Go, 12 760 bandes de 800×1280, bibliothèque `webtoon`, scan en 25 s) dans Chromium : 5 à 9 bandes dans la page, 17 Mo de mémoire JavaScript, défilement rapide sur 300 écrans fluide, retour à la bande 408 après rechargement, bandes jointives sans espace.
  - Corrigé grâce aux mesures : en fenêtre de 390 px à densité 1, chaque bande était demandée redimensionnée (480 px) et le serveur encodait une AVIF par bande traversée (408 requêtes en un défilement, 3,3 s avant les premières bandes) ; une bande est désormais servie en originale tant qu'elle ne fait pas plus de trois fois la largeur utile : 0 encodage, premières bandes en 0,9 s. Changement de largeur : la même position en pixels tombait sur une autre bande (408 → 199), corrigé et vérifié (163 → 163 → 163 en passant de 390 à 1440 puis 800 px). Nombres annoncés aux lecteurs d'écran formatés (« 12 760 »).
  - Recherches UI/UX Pro Max : virtualiser au-delà d'une cinquantaine d'éléments, charger au fil de l'eau, réserver la place.
  - Review Ponytail, corrigé : l'annonce aux lecteurs d'écran suit la bande retenue dans l'adresse (écrite à l'arrêt du défilement) au lieu de chaque bande traversée.
  - 95 tests Vitest, 99,5 % des lignes, 97,2 % des branches.
- [x] **3.8** `feat/reader-progress` : reprise, enregistrement, réglages retenus par série, tome suivant, zoom.
  - **Reprise** : sans `?page=` dans l'adresse, le lecteur ouvre la dernière page lue (`progress` du tome) ; un tome terminé (dernière page) repart du début. Les deux modes attendent le tome (réglages, progression) avant de s'afficher : plus d'ouverture fugace dans le mauvais sens.
  - **Enregistrement** (`reader/hooks.ts`, `useSaveProgress`) : `PUT /api/volumes/{id}/progress` 1 s après le dernier changement de page (feuilleter n'envoie que la page où l'on s'arrête), aussitôt quand l'onglet passe en arrière-plan ou que le lecteur se ferme, avec `keepalive`. En paginé, la dernière page de la vue (atteindre la fin marque le tome terminé) ; en webtoon, la dernière bande dès que son bas est à l'écran. Un échec : un seul message (notification), la lecture continue.
  - **Réglages par série** : le panneau Affichage (`chrome.tsx`, partagé par les deux modes, donc présent aussi en webtoon) ajoute le sens de lecture (Manga / BD / Webtoon) ; sens et disposition sont appliqués tout de suite et retenus par `PATCH /api/series/{id}/settings`. Décalage et coupe des doubles pages restent des réglages de la séance (propres au tome lu).
  - **Tome suivant** (ordre de la série) : en paginé, avancer après la dernière page montre les commandes, avec un bouton « Tome suivant » centré au-dessus de la barre ; en webtoon, un bloc « Fin du tome » sous la dernière bande (ou « Retour » sans tome suivant). Lecteur remonté à neuf à chaque tome (clé par identifiant).
  - **Zoom** (Q15) : double tap ×2 en paginé, sur le point touché, déplacement par le défilement natif (doigt, molette), image redemandée à la taille zoomée (nette) ; tourner la page ou un second double tap en sort. Le tap au centre attend 300 ms un éventuel second tap ; les taps des côtés tournent la page sans attendre (le double tap ne s'y amorce que zoomé).
  - Captures Playwright (binaire release, série de deux tomes dont le tome 67 en RAR solid) : zoom en 1280 et 390, fin de tome aux quatre tailles, panneau, fin du mode webtoon. Corrigé grâce aux captures : en 390 px, le bouton « Tome suivant » dans la barre écrasait le curseur à 70 px (déplacé sur sa propre ligne : 192 px) ; `pr-safe` / `pl-safe` annulaient la marge de 8 px de la barre (marge = la plus grande des deux) ; le bloc de fin du webtoon collait au bas de l'écran.
  - Recherches UI/UX Pro Max : `touch-action` (pas de zoom natif au double tap, qui entrerait en conflit), action explicite plutôt que passage automatique au tome suivant.
  - Review Ponytail, corrigé : zoomé, le pincement natif reste possible (`touch-action: manipulation`) ; un réglage dont l'enregistrement a échoué reste appliqué même quand le tome est redemandé au serveur (choix de la séance gardé par le lecteur, plus dans la copie en cache) ; variable de test inutile retirée.
  - 104 tests Vitest, 99,3 % des lignes, 96,1 % des branches.
- [x] **3.9** `test/reader-e2e` : Playwright dans la CI, invariant de lecture dans le navigateur, captures aux quatre tailles et corrections d'alignement.
  - Bibliothèque générée par `shiori-testkit` (`examples/e2e_library.rs`) : manga CBZ (couverture JPEG, deux doubles pages), manga RAR5 solid écrit à l'envers de l'ordre de lecture, webtoon de 60 bandes, et `manifest.json` (couleur attendue de chaque page). Couleurs tournant de l'angle d'or : deux pages voisines diffèrent d'au moins 139 sur une composante, la tolérance est de 24 (AVIF avec pertes).
  - `web/e2e` (`@playwright/test` 1.65, Chromium, `npm run e2e`) : un vrai `shiori serve` sur une base neuve, bibliothèques créées et scannées par l'API. **Invariant de lecture vérifié dans le navigateur** : la couleur lue dans chaque image affichée (canvas) est celle de sa position, sur toutes les vues d'un tome, en portrait (doubles pages coupées), en paysage (paires), pour le RAR solid et en webtoon jusqu'en bas ; chaque page est vue au moins une fois. Reprise après rechargement, tome suivant. Captures des quatre tailles (bibliothèques, bibliothèque, série, lecteur, panneau Affichage, webtoon) en artefact de la CI.
  - Job CI `e2e` (Linux, Q16), environ 2 min. Vitest limité à `src/` ; `tsconfig.e2e.json` vérifie les types des tests.
  - Corrigé grâce aux captures : l'anneau de focus de « Retour » était coupé en haut de l'écran (barre du lecteur sans marge verticale ; 4 px ajoutés en haut et en bas, ou la zone sûre).
  - Review Ponytail, corrigé : le test de reprise attend l'enregistrement de la page atteinte (et non celui de la page 1, parti pendant l'encodage de la première image) ; la préparation arrête le serveur si une étape échoue et échoue tout de suite si le serveur s'arrête (port déjà pris : vérifié) ; chemin des captures correct même avec un espace ; type simplifié.
  - Instabilité vue au premier passage en CI (réussi au second essai) : le test lisait l'annonce juste après la touche, alors que React Router change la page dans une transition ; sur la machine de la CI, plus lente, il lisait encore l'ancienne et croyait le tome fini. Les tests attendent maintenant le changement (même correction pour le test en paysage et celui de reprise), et la CI ne refait plus d'essai : une instabilité doit se voir.
  - Essai : six tests verts trois fois de suite en local. Le binaire de debug encode une page AVIF en 2 à 3 s : délais longs, les tests vérifient la justesse et non la vitesse.
- [x] **3.10** `chore/phase3-wrap` : essai réel sur tablette (ou sa taille), README à jour, bilan de phase, `/graphify . --update`.
  - **Allègement du front** (demandé le 2026-11-12). Le build de Vite minifiait déjà le JavaScript et le CSS.
    - Analyse du paquet (attribution des octets par la carte des sources) : `react-dom` 202 ko, `react-router` 92 ko, `i18next` 48 ko, `sonner`, TanStack Query et le virtualiseur environ 32 ko chacun ; pas de dépendance superflue, ce sont les briques de base.
    - **Compression des réponses** (`tower-http`, gzip et brotli en Rust pur, sans bibliothèque C) : JS principal 587 ko → 184 ko transférés, JSON des pages réduit de plus de 4 fois ; images exclues (déjà compressées). Test d'intégration.
    - **Lecteur chargé à la demande** (route `lazy` de React Router) : 76 ko (25 ko compressés) sortis de l'écran d'accueil (586 ko au lieu de 659). Si ce fichier manque (Shiori mis à jour pendant qu'un onglet était ouvert, serveur coupé) ou si l'écran plante, un écran traduit le dit sans présumer de la cause et propose de recharger, au lieu de l'erreur brute de React Router.
  - **Essai tactile à la taille de la tablette** (test e2e permanent, Chromium en 800×1280 avec écran tactile) : tap sur les côtés dans le sens manga, balayage au doigt, commandes au tap central, zoom au double tap, déplacement au doigt dans la page zoomée sans la tourner, sortie du zoom. Essai sur l'appareil réel laissé à l'utilisateur.
  - README : état, lancement (front intégré, déclaration d'une bibliothèque par l'API en attendant la phase 4), gestes du lecteur, tests du front.
  - Review Ponytail, corrigé : le texte de l'écran de secours ne présume plus d'une mise à jour (il couvre aussi un plantage de l'écran) ; le README précise que l'`id` de la bibliothèque à scanner est celui renvoyé à sa création.

### Bilan de la phase 3

- Livré :
  - **Front embarqué** dans le binaire (React 19, Vite 8, Tailwind 4, shadcn/ui), repli SPA, assets immuables, réponses compressées.
  - **Navigation provisoire** : bibliothèques, séries, tomes avec leur progression et leurs problèmes de scan.
  - **Interface bilingue** français / anglais, langue enregistrée sur le serveur, erreurs de l'API traduites d'après leur code stable, notifications.
  - **Lecteur paginé** : sens manga ou BD, page à la taille de l'écran (largeur fixe du serveur), erreurs à leur place, préchargement, clavier, tap, balayage, pincement ; **double page** (auto selon l'orientation, couverture seule, décalage, doubles pages coupées en deux moitiés en portrait) ; **zoom** au double tap.
  - **Mode webtoon** virtualisé, place réservée d'après le scan, essayé sur 12 760 bandes.
  - **Progression** enregistrée sur le serveur et reprise ; sens et disposition retenus par série ; tome suivant.
- Validé : webtoon réel de 2 Go (12 760 bandes), tome 67 en RAR solid ; **invariant de lecture vérifié dans le navigateur** (couleur de chaque page affichée) en portrait, paysage, RAR solid et webtoon ; captures aux quatre tailles à chaque PR.
- Qualité : 185 tests Rust, 105 tests Vitest (99,3 % des lignes, 96,1 % des branches), 7 tests de bout en bout dans Chromium ; CI Linux / Windows / macOS et e2e verte. 12 PR (#21 à #32), chacune passée en review Ponytail ; outils UI (UI/UX Pro Max, MCP shadcn, captures Playwright) sur chaque lot d'interface.
- Reste ouvert pour la suite :
  - le binaire de debug encode une page AVIF en 2 à 3 s (le binaire de release est rapide) : en développement, les premières pages d'un tome arrivent lentement ;
  - compression à la volée au niveau par défaut (brotli ≈ gzip) ; des fichiers précompressés au build gagneraient encore un peu ;
  - le bouton « Tome suivant » mène aussi à un tome marqué en erreur s'il suit dans la série ;
  - décalage et coupe des doubles pages ne sont pas retenus (réglages de la séance) ;
  - essai sur la tablette réelle à faire par l'utilisateur.

### Questions de la phase 3 (résolues le 2026-11-12)

- **Q11 Progression** ✅ : **côté serveur**, en base (un seul utilisateur implicite tant qu'il n'y a pas de comptes), pour reprendre la même page sur la tablette et sur le PC.
- **Q12 Mode par défaut** ✅ : **type de bibliothèque** choisi à sa création (`manga` → droite à gauche, `comics` → gauche à droite, `webtoon` → vertical) ; un réglage par série l'emporte.
- **Q14 Doubles pages en mode simple** ✅ : **coupées en deux moitiés successives par défaut** (« c'est pénible de tourner la tablette ») ; affichage en entier activable dans la barre d'outils.

Propositions retenues :

- **Q13 Disposition par défaut** : `auto` (double en paysage, simple en portrait), couverture seule.
- **Q15 Zoom** : double tap ×2 et déplacement en mode paginé, pincement natif en vertical, dans le lot 3.8.
- **Q16 Playwright dans la CI** : oui, job Linux seulement (navigateur Chromium, environ 1 à 2 min de plus par PR).
- **Q17 Découpage serveur des très hautes images** : reporté, aucun cas réel (voir les mesures) ; repli sur l'originale en attendant.
- **Q19 Langues** ✅ (2026-11-12) : français (source) et anglais.
- **Q20 Choix de la langue** ✅ : enregistré côté serveur, par un sélecteur dans l'interface ; global tant qu'il n'y a pas de comptes, par compte avec l'authentification.
- **Q21 Qui traduit** ✅ : l'interface, d'après les codes stables de l'API ; serveur et CLI restent en français (le message du serveur sert de secours). Pas de catalogue côté Rust.
- **Q18 Breakpoint tablette 8" en portrait** (CLAUDE.md) : sans objet en phase 3 (le lecteur en est exclu, la navigation provisoire est une simple liste) ; à valider en phase 4.

## Phase 4 : bibliothèque, navigation et administration (plan validé le 2026-11-12, terminée le 2026-11-13)

### Objectif

Remplacer la navigation provisoire par une vraie bibliothèque façon Plex (grille de couvertures, couverture de repli, « Continuer la lecture », recherche, filtres et tris), poser la navigation définitive (sidebar à partir de `lg`, barre d'onglets en bas en dessous) et piloter bibliothèques et scans depuis l'interface, sans terminal ni appel direct à l'API. `DESIGN_SYSTEM.md` fixe les règles visuelles que ces écrans appliquent.

Hors périmètre : métadonnées web et auteur (phase 5 ; la couverture de repli n'affiche que le titre en attendant), statistiques (Q26, reportées), authentification et rôles (« Plus tard »), scans planifiés.

### Existant réutilisé

- API : bibliothèques (liste, création, suppression), séries d'une bibliothèque avec couverture, détail d'une série avec la progression par tome, toute l'administration des scans (lancer, en cours, annuler, historique, détail, journaux paginés). Miniatures JPEG déjà servies (`?w=320&f=jpeg`, largeurs fixes).
- Front : cadre commun, i18n, client typé, composants shadcn Badge, Button, Popover, Progress, Select, Skeleton, Slider, Sonner, ToggleGroup.

### Base : migration `0006_library.sql`

```sql
-- Statut de publication, réglé à la main en phase 4, rempli par les métadonnées en phase 5 (Q23).
ALTER TABLE series ADD COLUMN publication_status TEXT
  CHECK (publication_status IN ('ongoing','completed','hiatus','cancelled'));  -- NULL = inconnu
-- Tri « Récemment ajoutés » (Q27). Écrit explicitement par le scan à la création de la série ;
-- les séries existantes prennent la date de création de leur bibliothèque.
ALTER TABLE series ADD COLUMN created_at TEXT;
UPDATE series SET created_at = (SELECT created_at FROM libraries WHERE libraries.id = series.library_id);
```

### Routes ajoutées ou modifiées

| Méthode | Route | Rôle |
|---|---|---|
| GET | `/api/series` | toutes les séries (filtre `library_id`) : couverture, `volume_count`, `read_count`, `reading_status` (`unread` / `in_progress` / `read`, calculé), `publication_status`, `created_at`, `last_read_at`, `resume_volume_id` (tome qu'ouvre « Lire ») |
| GET | `/api/libraries/{id}/series` | même résumé enrichi (requête partagée) |
| GET | `/api/continue?limit=20` | tomes commencés non terminés, plus le tome suivant d'une série dont le dernier tome lu est terminé (tomes en erreur sautés), du plus récent au plus ancien (Q24) |
| PATCH | `/api/series/{id}` | `{ publication_status }` (`null` = inconnu) ; la phase 5 l'étendra aux métadonnées |
| — | `GET /api/admin/scans/{id}/logs` | chaque ligne gagne `series_id` et `volume_id`, retrouvés par le chemin (jointure, sans migration) |

Recherche et filtres **dans le navigateur** sur la liste complète (une requête, résultat immédiat à la frappe), insensibles à la casse et aux accents ; `shortcut:` dans le code : passer côté serveur au-delà d'environ 5 000 séries.

### Navigation

- **À partir de `lg` (1024 px)** (Q18) : sidebar fixe, rétractable en icônes (Sidebar shadcn, état gardé dans `localStorage`) : recherche globale, Accueil, Bibliothèques (chacune dessous), Séries, Administration ; langue en pied. Pas d'entrée Statistiques tant que la page n'existe pas (Q26).
- **En dessous** : barre d'onglets fixe en bas, au-dessus de la zone sûre : Accueil, Séries, Recherche, Administration. La tablette 8" a les onglets en portrait (800 px, toute la largeur pour la grille) et la sidebar en paysage (1280 px).
- **Routes** : `/` (Continuer la lecture + une étagère par bibliothèque), `/libraries/:id` (grille), `/series?q=&read=&status=&library=&sort=` (grille filtrée, sert aussi d'écran de recherche, filtres dans l'adresse), `/series/:id`, `/admin/libraries`, `/admin/scans`, `/admin/scans/:id`. Bouton « Retour » explicite sur les pages de détail.
- Tris : nom, dernière lecture, récemment ajoutés.

### Tests

- **Rust** : migration 0006 (séries existantes datées), statuts de lecture (aucun tome ouvert, en cours, tous lus), ordre de « Continuer » et tome suivant (tome en erreur sauté), `PATCH` valide et invalide, jointure des journaux (fichier disparu : `null`). Couverture ≥ 80 % lignes et branches.
- **Vitest** : filtres et recherche (fonctions pures, accents), couverture de repli sur image absente ou en erreur, navigation (sidebar / onglets), formulaires d'administration (erreurs traduites), suivi du scan en cours. Seuils 80 %.
- **Bout en bout** : créer une bibliothèque et la scanner par l'interface, grille, recherche, filtre, série, lecteur ; captures aux quatre tailles en artefact.

### Lots (une branche + une PR chacun)

Pour chaque lot d'interface : skill UI/UX Pro Max avant de coder et en contrôle final, composants par le MCP shadcn (import `cn` vérifié, liste de contrôle du MCP), captures Playwright en 390×844, 800×1280, 1280×800 et 1440×900 sur le binaire release puis corrections, review Ponytail avant la PR.

- [x] **4.0** `docs/phase4-plan` : ce plan.
- [x] **4.1** `docs/design-system` : `DESIGN_SYSTEM.md` (skill design-system) : couleurs du thème existant, typographie, espacements, breakpoints, contrastes mesurés, cibles de 44 px, focus ambre, mouvements réduits ; carte de couverture, couverture de repli, barre de progression, états vide / chargement / erreur ; règles shadcn.
  - [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md), rédigé avec la skill design-system d'UI/UX Pro Max et ses règles UX (chargement à la demande, place réservée, états vides, troncature, navigation active). Sa palette générique (bleu marine, polices web) écartée : la direction de CLAUDE.md prime.
  - Contrastes recalculés : le texte secondaire tombe à 4,1:1 sur `#3F3F46`, d'où la règle « pas de texte secondaire sur une surface plus claire que `#2A2A2A` ». Couverture de repli : dégradé d'une teinte tirée du titre, 16 % de luminosité au plus (texte secondaire ≥ 4,75:1 pour toute teinte, 3,9:1 à 20 %).
  - Grille de couvertures sans breakpoint (`auto-fill`, 9 rem au moins) : 2 colonnes en 390 px, 5 en 800, 6 en 1280 avec sidebar, 7 en 1440. Ambre écrit `bg-primary` (`#E5A00D`), jamais `amber-500` (`#F59E0B`).
  - Le vérificateur de la skill (`validate-tokens`) ne trouve des couleurs en dur que dans `index.css`, où elles sont définies.
  - Review Ponytail, corrigé : la piste de la barre de progression sur une couverture passe de `bg-black/60` à `bg-black/80` (2,6:1 sur une couverture blanche, sous le seuil de 3:1).
- [x] **4.2** `feat/app-shell` : sidebar et barre d'onglets, routes en place, langue déplacée dans la sidebar et l'administration, tests et e2e mis à jour.
  - Sidebar shadcn (MCP) réduite à ce qui sert : affichée à partir de `lg` seulement, donc sans le panneau mobile ni son hook ; état réduit dans `localStorage` (cookie inutile sans rendu serveur ; stockage refusé : sidebar ouverte, testé) ; pas de Ctrl+B (déjà pris par Firefox) ; entrées de 44 px (largeur réduite 3,75 rem) ; focus ambre global au lieu de l'anneau shadcn ; entrée active par l'`aria-current` du `NavLink`, en ambre. Couleurs de la sidebar prises dans le thème (le registre ajoutait une palette claire et bleue).
  - Barre d'onglets sous `lg` : icône et libellé, onglet actif en ambre, hauteur dans `--tabbar-height` (nulle à partir de `lg`), dont dépendent la marge du bas du contenu et la position des notifications.
  - **Onglets et entrées limités aux écrans qui existent** : Accueil (avec la liste des bibliothèques dans la sidebar) et Administration. Séries et Recherche arrivent avec leur écran au lot 4.5 (pas de lien vers une page vide, comme pour Q26).
  - Page Administration (`/admin`) : langue de l'interface pour l'instant ; le sélecteur reste aussi en pied de sidebar. Lien « Aller au contenu » au premier Tab.
  - Le registre shadcn a de nouveau installé le paquet npm `cn` et écrit `import { cn } from "cn"` dans cinq fichiers : corrigé. Le test du thème couvre aussi les couleurs `sidebar-*`.
  - Captures Playwright aux quatre tailles (binaire release, bibliothèque au nom très long, français et anglais) ; corrigé grâce à elles : le lien « Aller au contenu » perdait ses marges au focus (`not-sr-only` les remet à zéro) et chevauchait le logo. Page Administration ajoutée aux captures e2e.
  - Recherches UI/UX Pro Max : lien d'évitement, état actif visible, contenu jamais caché par une barre fixe, retour au toucher.
  - Review Ponytail, corrigé : sidebar dépliée, l'infobulle masquée restait reliée à l'entrée (`aria-describedby`) et les lecteurs d'écran lisaient le libellé deux fois ; elle n'existe plus que sidebar réduite (testé).
  - Reste : dans le lecteur sur mobile, une notification monte de 56 px de trop (`--tabbar-height` est globale alors que le lecteur n'a pas de barre d'onglets) ; cosmétique.
  - 110 tests Vitest, 99,2 % des lignes, 96 % des branches ; 7 tests e2e verts.
- [x] **4.2b** `feat/logo` (ajouté le 2026-11-12) : logos fournis par l'auteur.
  - Dans `web/public/` : favicon ICO (32 et 48 px) et SVG (crème quand l'OS est sombre), icône d'écran d'accueil iOS (180 px), icônes Android 192 / 512 et maskable 512, et la marque en deux variantes. Logo clair en tête de la sidebar (décoratif, à côté du nom), logo dans le README selon le thème de GitHub.
  - Manifeste minimal (`manifest.webmanifest` : nom, icônes, `standalone`, fond `#121212`) pour que les icônes Android servent dès maintenant ; la PWA finalisée (hors ligne, installation guidée) reste en phase 6. Types servis vérifiés (`application/manifest+json`, `image/x-icon`).
  - DESIGN_SYSTEM.md : section « Logo » ; ses couleurs (bleu nuit, crème, rouge) sont réservées au logo, l'interface garde son ambre.
  - Captures aux quatre tailles, focus clavier sur le logo.
- [x] **4.3** `feat/library-api` : migration 0006, `GET /api/series`, résumé enrichi, `GET /api/continue`, `PATCH /api/series/{id}`, `created_at` écrit par le scan.
  - Migration `0006_library.sql` : `publication_status` (`NULL` = inconnu) et `created_at` des séries, écrit par le scan à la création ; les séries existantes prennent la date de leur bibliothèque (testé sur une base migrée depuis la version 5).
  - Résumé de série (`GET /api/series`, `GET /api/libraries/{id}/series`, même requête) : `library_id`, `read_count`, `reading_status` (`unread` si aucun tome ouvert, `read` si tous lus jusqu'au bout, `in_progress` sinon ; un tome en erreur jamais lu laisse la série « en cours »), `publication_status`, `created_at`, `last_read_at`, `resume_volume_id`. `?library_id=` inconnu : 404 ; paramètre inconnu : `400 invalid_query`. Toujours sans pagination (`shortcut:` : recherche et filtres côté client).
  - **Suite de la lecture** (une règle partagée par « Lire » et « Continuer ») : le tome ouvert en dernier s'il n'est pas arrêté sur sa dernière page, sinon le suivant dans l'ordre ni lu ni en erreur. « Lire » retombe ensuite sur le premier tome non lu, puis sur le premier tome (relire).
  - `GET /api/continue?limit=` (20 par défaut, 100 au plus) : une entrée par série commencée qui a une suite, de la lecture la plus récente à la plus ancienne ; le tome en cours porte sa progression, le tome suivant n'en a pas.
  - `PATCH /api/series/{id}` : `{ publication_status }`, champ absent = inchangé, `null` = inconnu, valeur ou champ inconnu : `400 invalid_body` ; renvoie le détail de la série, qui gagne `publication_status`.
  - Types TypeScript alignés (`SeriesSummary`, `ContinueItem`, `ReadingStatus`, `PublicationStatus`).
  - Review Ponytail, corrigé : relire un tome déjà fini proposait le tome suivant (le drapeau `finished` reste vrai pour toujours) ; la suite se décide sur la page où l'on s'est arrêté (testé).
  - Couverture : 97,4 % des lignes, 91,6 % des branches.
- [x] **4.4** `feat/library-grid` : `CoverCard` (`aspect-[2/3]`, miniatures JPEG à la demande avec `srcset` 160/320/480, barre `h-1`, survol : zoom léger, liseré, bouton « Lire »), couverture de repli (dégradé sombre + titre) si absente ou en erreur, accueil avec le carrousel « Continuer la lecture » (défilement natif accroché, sans barre visible) et les étagères, page bibliothèque en grille ; données de test à titres très longs et couvertures manquantes.
  - `library/covers.tsx` : `CoverCard` (un seul lien, texte d'abord dans le DOM pour que le lecteur d'écran annonce le titre avant la barre de progression, couverture au-dessus à l'écran par `flex-col-reverse`), miniatures JPEG (`srcset` 160 / 320 / 480, `sizes` d'après la grille, chargement différé sauf le premier écran), barre `h-1` sur piste `bg-black/80`, badge « Lu » ou « Suivant », bouton « Lire » au survol (absent au toucher, visible au clavier). `SeriesCard` (tomes lus, ouvre `resume_volume_id`) et `ContinueCard` (tome en cours avec sa page, ou tome suivant).
  - Couverture de repli si la série n'a pas de couverture ou si l'image ne se charge pas : dégradé d'une teinte tirée du titre (`titleHue`), titre sur quatre lignes au plus.
  - Accueil : « Continuer la lecture » (section absente s'il n'y a rien), puis une étagère par bibliothèque, ses 20 derniers ajouts (tri par `created_at`, Q27) et « Tout voir ». Étagères en défilement natif accroché, sans barre visible (utilitaire `scrollbar-none`). Squelettes en forme de couvertures (`QueryState` accepte un `loading`).
  - Page bibliothèque en grille `auto-fill` (2, 5, 6 et 7 colonnes aux quatre tailles mesurées), titre complet au survol. L'écran provisoire des bibliothèques en liste est retiré.
  - Le lecteur périme aussi `['libraries']` et `['continue']` quand il enregistre la progression : sans cela, l'accueil montrait jusqu'à 30 s une page dépassée au retour du lecteur (trouvé en relisant le cache ; testé avec la durée de validité de la production, le test échoue sans le correctif).
  - Captures Playwright aux quatre tailles (binaire release ; titres très longs, série sans couverture, tomes en cours et suivants), survol et clavier. Corrigé grâce à elles : le nom accessible des cartes commençait par la barre de progression ; pluriel « 1 tomes lus ».
  - 115 tests Vitest, 99,1 % des lignes, 96 % des branches ; e2e verts.
- [x] **4.5** `feat/library-filters` : recherche, filtres statut de lecture / statut de publication / bibliothèque, tris (nom, dernière lecture, récemment ajoutés), le tout dans l'adresse ; champ relié à la sidebar et à l'onglet.
  - Écran `/series` (toutes les séries) : recherche à la frappe, insensible à la casse et aux accents, mot par mot (« piece one » trouve « One Piece ») ; filtres lecture (non lues, en cours, lues), publication (dont « statut inconnu »), bibliothèque (masqué s'il n'y en a qu'une) ; tri par titre, dernière lecture (jamais lues à la fin) ou ajout (Q27). Tout est dans l'adresse (`?q=&read=&status=&library=&sort=`, valeurs inconnues ignorées), réécrite sans remplir l'historique ; « Retour » depuis une série retrouve les filtres. Liste déroulante en ambre quand elle restreint la liste, « Effacer les filtres » (le tri reste), nombre de résultats annoncé (`aria-live`), état « aucune série ne correspond » qui propose d'effacer.
  - Logique en fonctions pures testées (`library/filters.ts`) ; `shortcut:` de la phase : recherche côté client (Q26).
  - `/search` : le même écran, champ au focus à l'ouverture. Navigation à quatre entrées (Accueil, Séries, Recherche, Administration), sidebar et onglets.
  - Input shadcn (MCP) : le registre a encore installé le paquet `cn` (retiré) ; adapté à 44 px, 16 px de texte (pas de zoom iOS), focus ambre global. Libellé visible au-dessus du champ, `type="search"`, `enterKeyHint`.
  - Le retour de la page série portait le même nom (« Séries ») que la nouvelle entrée de navigation, vers un autre écran : il porte désormais le nom de la bibliothèque (« Accueil » si la série est inconnue).
  - Captures aux quatre tailles. Corrigé grâce à elles : filtres sur trois lignes en 390 px (deux colonnes désormais), libellés coupés sans points de suspension.
  - Review Ponytail, corrigé : le champ était piloté par l'adresse, que React Router change dans une transition (lettres en retard, curseur perdu sur appareil lent) ; texte tapé gardé dans un état local. Vérifié dans Chromium avec le processeur ralenti six fois : aucune lettre perdue, curseur en place après une correction au milieu du mot.
  - Tests instables du lecteur (premier test d'un fichier, vus deux fois pendant la couverture) : la route `lazy` du lecteur, chargée à froid, dépassait le délai d'attente ; elle est préchargée avant les tests.
  - 125 tests Vitest, 98,8 % des lignes, 95,4 % des branches ; e2e verts (capture de l'écran Séries ajoutée).
- [x] **4.6** `feat/series-detail` : page série refaite : couverture, statut de publication modifiable, progression de la série, bouton principal « Commencer » / « Reprendre tome N », tomes en grille de couvertures avec badges, marquer lu / non lu, problèmes de scan d'un tome avec leurs codes traduits (catalogues des codes de diagnostic, absents aujourd'hui), ancre par tome.
  - En-tête : couverture (repli si absente), titre sur trois lignes, « N tomes · M lus » et barre de progression, bouton principal « Reprendre · Tome N » (arrêté avant la dernière page), « Relire » (tout est lu) ou « Lire », statut de publication modifiable (`PATCH /api/series/{id}`, la liste revient à la valeur du serveur et une notification le dit en cas d'échec).
  - Serveur : le détail d'une série gagne `resume_volume_id`, calculé par la même règle que la liste (une seule règle, testée).
  - Tomes en grille de couvertures : badge « Erreur » (fond rouge foncé), « Avertissement » ou « Lu », barre et « page / total » pour un tome en cours, ancre `#volume-<id>` pour les liens des journaux (lot 4.8 : React Router ne fait pas défiler jusqu'à une ancre lors d'une navigation interne, le lien devra le faire). Menu par tome (pastille de 36 px, zone de 44 px, toujours visible au toucher) : marquer lu (dernière page enregistrée, comme le lecteur), marquer non lu (`DELETE` de la progression), problèmes du scan.
  - Fenêtre « Problèmes du scan » : problèmes du dernier scan du tome, gravité et texte traduits d'après le code stable (`issueText`, les 20 codes de `IssueCode` en français et en anglais ; un code inconnu garde le message du serveur), entrée concernée.
  - Composants par le MCP shadcn : DropdownMenu et Dialog, réduits à ce qui sert ; entrées de 44 px, focus ambre global, bouton de fermeture de 44 px traduit (le registre écrivait « Close » en dur). Le paquet `cn` est encore revenu (retiré). Badge, `Row` et `RowTitle` retirés (plus utilisés).
  - Captures aux quatre tailles (série au titre très long, tome 67 en RAR solid, tome sans image en erreur, menu, fenêtre). Corrigé grâce à elles : le retour au nom de bibliothèque très long élargissait la page en 390 px (tronqué désormais), le badge « Avertissement » chevauchait le menu sur une carte étroite, le nom du fichier était coupé au milieu d'un mot.
  - `mockApi` transmet la requête à la réponse simulée (GET, PATCH et DELETE sur une même adresse dans les tests).
  - 130 tests Vitest, 99 % des lignes, 95,5 % des branches ; e2e verts.
- [x] **4.7** `feat/admin-libraries` : liste, création (Dialog : nom, chemin en champ texte validé par le serveur (Q25), type ; `invalid_root` et doublon traduits), suppression (AlertDialog : fichiers et progression conservés), « Scanner » par bibliothèque.
  - Section « Bibliothèques » de `/admin` (une page d'administration à sections plutôt qu'une route `/admin/libraries` : moins d'écrans, même accès) : nom, type, nombre de séries et de tomes, dossier en entier. « Scanner » par bibliothèque (`POST /api/admin/scans`), résultat dit par une notification (`scan_already_running` traduit).
  - Création dans une fenêtre : nom, dossier (champ texte validé par le serveur, Q25, aide avec un exemple de chemin), type (qui donne le sens de lecture). Libellés visibles ; erreurs du serveur traduites sous le champ concerné (`invalid_name`, `name_taken` sous le nom ; `invalid_root`, `root_overlaps` sous le dossier, reliées par `aria-describedby` et `aria-invalid`), les autres en tête ; bouton occupé pendant l'envoi. **Le scan démarre dès la création** (une bibliothèque neuve est vide tant qu'elle n'est pas scannée).
  - Suppression après confirmation (AlertDialog shadcn par le MCP, réduit) qui dit ce qui est conservé : fichiers intacts, progression gardée.
  - Accueil sans bibliothèque : bouton « Ajouter une bibliothèque » vers l'administration (le texte renvoyait à l'API).
  - Captures aux quatre tailles (liste, fenêtre avec une erreur de dossier, confirmation). Parcours réel dans Chromium : chevauchement refusé et dit sous le champ, création sur un vrai dossier, scan lancé, une série trouvée, suppression.
  - Le paquet `cn` est encore revenu avec AlertDialog (retiré).
  - 136 tests Vitest, 99,1 % des lignes, 95,3 % des branches.
- [x] **4.8** `feat/admin-scans` : passe en cours (interrogée chaque seconde tant qu'elle tourne, progression, fichier courant, annuler), options `deep` / `rehash` expliquées, historique filtrable (statut, bibliothèque), détail et journaux paginés (filtre de niveau, codes traduits, lien vers le tome en erreur), jointure des journaux côté API, listes de séries rafraîchies en fin de scan.
  - Serveur : chaque ligne du journal gagne `series_id` et `volume_id` (jointure par chemin sur les emplacements, sans migration). Défaut trouvé en chemin : le journal écrivait le chemin tel quel (`\` sous Windows) alors que les emplacements utilisent la clé absolue à `/` ; il écrit désormais la même clé (principe 9). Testé, y compris un chemin qui n'est plus suivi.
  - **Suivi global** (monté dans le cadre commun) : la passe en cours est redemandée chaque seconde seulement pendant un scan ; à la fin, bibliothèques, séries, « Continuer », tomes et historique sont redemandés, et une notification dit comment le scan s'est terminé (terminé, annulé, échec, interrompu) avec ses compteurs. Lancer un scan relance ce suivi.
  - Carte « Scan en cours » (administration et page des scans) : bibliothèque, barre de progression, « N fichiers sur M » (« liste en préparation » avant), compteurs (erreurs en rouge), fichier courant, « Annuler le scan » puis « Annulation… ». Affichée, elle interroge aussi toutes les 10 s au repos, pour voir un scan lancé depuis un autre appareil.
  - Page `/admin/scans` : lancer un scan avec ses options, chacune expliquée (contrôle approfondi, relire tous les fichiers ; Checkbox shadcn par le MCP, 20 px dans une ligne de 44 px), bouton désactivé et raison écrite pendant un scan ; historique filtrable par statut et bibliothèque (dans l'adresse), 20 par page, « Charger la suite ».
  - Page `/admin/scans/:id` : statut, bibliothèque, cible, origine, début, durée (`Intl`, « 1 min 32 s »), fichiers, options, compteurs, erreur fatale ; journal filtrable par niveau, 100 lignes par page, codes traduits (`issueText`), message du serveur sinon (Q21), lien « Voir le tome » vers `/series/<id>#volume-<id>`. La page série fait défiler jusqu'au tome et lui donne le focus (React Router ne le fait pas). Vérifié dans Chromium.
  - Captures aux quatre tailles, dont un **scan réel du webtoon de 2 Go** pendant qu'il tourne (environ 1 min 30 s, l'avancement compte les fichiers), sa notification de fin, l'historique et le détail. Corrigé grâce à elles : espace avant les deux-points codée en dur (juste en français, fausse en anglais), désormais dans les catalogues.
  - Review Ponytail, corrigé : un scan lancé depuis un autre appareil n'apparaissait pas sur la carte (interrogation au repos ajoutée) ; un filtre de bibliothèque devenait impossible à retirer après la suppression de cette bibliothèque. Testés.
  - Test instable du lot 4.3 révélé par la CI : deux lectures enregistrées dans la même milliseconde ont la même date, et la suite de lecture prenait l'autre tome. Le test espace ses lectures de quelques millisecondes (un vrai lecteur n'enregistre jamais deux tomes dans la même milliseconde).
  - 147 tests Vitest, 98,6 % des lignes, 94 % des branches ; e2e verts.
- [x] **4.9** `chore/phase4-wrap` : e2e du parcours complet par l'interface, bilan de phase, README, `graphify update`, scratchpad vidé.
  - **Parcours complet de bout en bout** (`e2e/library.spec.ts`, Chromium, sans appel direct à l'API) : créer une bibliothèque, voir son scan lancé puis annoncé terminé, la retrouver sur l'accueil et en grille, la chercher par l'onglet du bas en 390 px (sans accents ni majuscules), filtrer jusqu'à « aucune série », effacer, ouvrir la série, lire le tome 1, supprimer la bibliothèque. Dossier et nom propres à chaque exécution ; trois passages de suite verts.
  - **Défaut trouvé par ce test** : un scan plus court qu'un aller-retour (une petite bibliothèque se scanne en quelques dizaines de millisecondes) finissait avant que le suivi le voie en cours : ni notification de fin, ni listes rafraîchies. Le lancement dépose désormais le numéro de la passe dans le cache, et le suivi réagit à chaque réponse du serveur, pas seulement aux changements de valeur. Test Vitest qui échoue sans le correctif.
  - README à jour (état de la phase 4, création des bibliothèques par l'administration au lieu de l'API), DESIGN_SYSTEM.md (composants ajoutés pendant la phase), graphe Graphify mis à jour, scratchpad vidé.

- [x] **4.10** `feat/series-progress` (ajouté le 2026-11-13, retours de l'utilisateur après un premier essai) : avancement en pages, badges cumulés, scan depuis la sidebar.
  - Serveur : résumé et détail d'une série gagnent `page_count` et `pages_read` (un tome fini compte en entier, un tome en cours jusqu'à la page atteinte), une seule fonction pour les deux. Testé.
  - Barre des cartes de série en pages, pourcentage sur la page série (« 1 tome lu sur 2 · 40 % »), arrondi vers le bas : « 100 % » veut dire « fini » (à la bande 12 700 sur 12 760, « 99 % »). Un webtoon d'un seul tome, commencé, a désormais sa barre, y compris dans la recherche.
  - Badges cumulés sur un tome : « Avertissement » (ou « Erreur ») et « Lu » ensemble, empilés.
  - Bouton « Scanner » à côté de chaque bibliothèque dans la sidebar (44 px, nom accessible), la fin étant annoncée par le suivi des scans.
  - Captures aux quatre tailles (recherche filtrée sur « en cours », page du webtoon à 40 %, sidebar). Review Ponytail, corrigé : la règle de comptage était écrite deux fois (serveur et page série) ; un « 100 % » s'affichait avant la fin.
  - 151 tests Vitest, 98,6 % des lignes, 93,9 % des branches.
- [x] **4.11** `feat/page-turn` (ajouté le 2026-11-13) : réglage « Tourner les pages » séparé du sens de lecture : comme le sens de lecture, de gauche à droite, de droite à gauche (trois choix, utile aux gauchers). Il pilote les zones de tap, le balayage, les flèches du clavier et le sens du curseur ; le sens de lecture garde l'ordre des doubles pages et des moitiés coupées. Valeur par défaut globale dans l'administration (comme la langue, par compte plus tard), réglage par série qui l'emporte (panneau « Affichage »).
  - Migration `0007_page_turn.sql` : `app_settings.page_turn` (défaut `reading`) et `series.page_turn` (`NULL` = défaut global). `ReadingSettings` gagne `page_turn` et `page_turn_override`, lus par une seule liste de colonnes (`settings_columns!`) dans les trois requêtes qui les construisent.
  - `PUT /api/settings` devient partiel : champ absent = inchangé (la langue n'est plus obligatoire), corps vide refusé, `page_turn: null` refusé (ce réglage a toujours une valeur). `PATCH /api/series/{id}/settings` accepte `page_turn` (`null` = défaut global).
  - Lecteur : le sens des pages (`turn`) pilote les taps, le balayage, les flèches du clavier, les boutons page précédente / suivante et le curseur ; le sens de lecture (`direction`) garde l'ordre des pages dans les vues (paire de droite à gauche en manga, moitié droite d'abord). Panneau « Affichage » : « Tourner les pages », trois choix en colonne (libellés « Toucher à droite avance », « Toucher à gauche avance »), retenu pour la série.
  - Administration, section « Lecture » : valeur par défaut, avec son explication ; un changement redemande les réglages des séries et des tomes.
  - Captures aux quatre tailles (panneau, administration). Corrigé grâce à elles : en colonne, les choix étaient écrasés à 20 px de haut (`flex-1`), désormais 44 px. Essai réel dans Chromium : en manga avec « toucher à droite avance », la flèche droite passe aux pages 2 et 3, réglage enregistré pour la série.
  - Review Ponytail, corrigé : `page_turn: null` était ignoré sans erreur.
  - 153 tests Vitest, 98,6 % des lignes, 93,7 % des branches ; tests Rust (défaut global, réglage de série, `PUT` partiel).

- [x] **4.12** `feat/logo-v2` (ajouté le 2026-11-13) : deuxième version des logos fournie par l'auteur (cases de manga et marque-page ambre).
  - Les huit fichiers de `web/public/` remplacés. Le nommage s'inverse : `shiori-mark.svg` est désormais la version pour fond sombre (crème et ambre), `shiori-mark-light.svg` celle pour fond clair (noir et ambre foncé) ; la sidebar et le README suivent. Le favicon SVG suit le thème de l'OS.
  - DESIGN_SYSTEM.md : section « Logo » à jour ; ses couleurs restent réservées au logo.
  - Un mode clair de l'interface (couleurs, contrastes, captures doublées, choix enregistré) est un lot à part, à planifier.
  - Captures aux quatre tailles ; types servis vérifiés.

### Bilan de la phase 4

- Livré :
  - **Navigation définitive** : sidebar réductible à partir de `lg`, barre d'onglets en dessous (Accueil, Séries, Recherche, Administration), lien d'évitement ; logos de l'auteur (favicon, icônes d'écran d'accueil, manifeste).
  - **Bibliothèque façon Plex** : accueil avec « Continuer la lecture » (tome en cours ou suivant) et une étagère des derniers ajouts par bibliothèque ; grilles de couvertures, couverture de repli générée ; page série (reprise, statut de publication, marquer lu ou non lu, problèmes du scan traduits).
  - **Recherche et filtres** : sans accents, mot par mot ; lecture, publication, bibliothèque ; tri par titre, dernière lecture ou ajout ; tout dans l'adresse.
  - **Administration** : bibliothèques (création suivie d'un scan, suppression confirmée), scan en cours (progression, annulation, vu depuis n'importe quel appareil), options expliquées, historique filtrable, journal traduit avec lien vers le tome.
  - **API** : `GET /api/series`, `GET /api/continue`, `PATCH /api/series/{id}`, résumé de série enrichi, tome à reprendre (une seule règle), journal relié aux tomes ; migration 0006.
  - **DESIGN_SYSTEM.md** : couleurs et contrastes mesurés, grille sans breakpoint, carte de couverture, couverture de repli, composants et états.
- Validé : parcours complet par l'interface dans Chromium ; scan réel du webtoon de 2 Go suivi dans l'interface ; captures aux quatre tailles à chaque lot ; saisie de la recherche vérifiée avec un processeur ralenti six fois.
- Qualité : 192 tests Rust, 148 tests Vitest (98,6 % des lignes, 94 % des branches), 8 tests de bout en bout ; 11 PR (#34 à #44), chacune passée en review Ponytail ; outils UI (UI/UX Pro Max, MCP shadcn, captures Playwright) sur chaque lot d'interface.
- Défauts trouvés et corrigés en route : relire un tome fini proposait le suivant ; listes de l'accueil périmées au retour du lecteur ; champ de recherche piloté par une transition ; chemins du journal au mauvais format sous Windows ; scan trop court jamais vu ; deux tests instables (route `lazy` chargée à froid, dates à la milliseconde).
- Reste ouvert pour la suite :
  - le registre shadcn installe le paquet npm `cn` et écrit `import { cn } from "cn"` à chaque ajout (corrigé à chaque fois, la règle oxlint le bloque) ;
  - dans le lecteur sur mobile, une notification monte de 56 px de trop (`--tabbar-height` globale) ;
  - le journal d'une passe en cours ne se met pas à jour en continu (il se rafraîchit à la fin) ;
  - les messages du serveur sans code (lignes « scanné » du journal) restent en français (Q21) ;
  - essai sur la tablette réelle à faire par l'utilisateur ;
  - test e2e instable vu une fois en CI (PR du lot 4.12, qui ne touche pas au lecteur ; vert au second essai et en local) : « reprend à la page lue… puis passe au tome suivant », le bouton « Tome suivant » n'est jamais apparu après la flèche en fin de tome. Cause non trouvée ; à surveiller, et à instrumenter s'il revient.

### Questions de la phase 4 (résolues le 2026-11-12)

- **Q18 / Q22 Breakpoint tablette 8" en portrait** ✅ : `lg` (1024 px) ; onglets en bas en portrait, sidebar en paysage.
- **Q23 Statut de publication** ✅ : colonne réglée à la main sur la page série, remplie par les métadonnées en phase 5.
- **Q24 « Continuer la lecture »** ✅ : tomes en cours + tome suivant d'une série dont on vient de finir un tome, tomes en erreur sautés.
- **Q25 Chemin d'une bibliothèque** ✅ : champ texte validé par le serveur ; pas d'explorateur de dossiers (il exposerait le disque sans authentification).
- **Q26 Statistiques** ✅ : reportées, pas d’entrée dans la sidebar en attendant.
- **Q27 Tri « Récemment ajoutés »** ✅ : oui, `series.created_at` dans la migration 0006.

## Phase 5 : métadonnées web et édition manuelle (plan validé le 2026-11-13, terminée le 2026-11-13)

### Objectif

Métadonnées des séries et des tomes depuis les fichiers (ComicInfo.xml, `index.json`) et le web (AniList, MangaDex), édition manuelle avec verrouillage par champ, identification façon Plex (choix de la source, grille de couvertures des candidats), couvertures externes, et séries qui survivent au renommage de leur dossier.

Hors périmètre : MangaUpdates et Comic Vine (clé d'API, usage non commercial), à ajouter plus tard derrière le même trait `Source` ; actualisation périodique automatique.

### Principes

1. **Le scan reste hors ligne et rapide** : il ne lit que les sources locales. Le web passe par une tâche à part, jamais pendant un scan.
2. **Verrouillage par champ** : éditer un champ le verrouille, aucune source ne l'écrase ; le déverrouiller recalcule la valeur depuis les sources. Chaque source garde sa dernière réponse en base : refusionner (verrou retiré, ordre ou langue changés) ne fait aucun appel réseau.
3. **Le scan garde l'identité, l'ordre, le numéro et les pages des tomes** (principe 3) : un `Number` ComicInfo différent est affiché, jamais utilisé pour réordonner.
4. **Rien de personnel envoyé** : seul le titre cherché part ; aucun chemin, nom de fichier ni nom d'utilisateur ; agent HTTP `Shiori/<version>`.

### Existant réutilisé

- `series.publication_status` (Q23) et `PATCH /api/series/{id}` (lot 4.3), étendus aux autres champs.
- La couverture de repli et la carte de série prévoient déjà la ligne auteur (DESIGN_SYSTEM.md).
- Le scan voit déjà `ComicInfo.xml` (code `ignored_entry`) ; l'`index.json` d'une application de téléchargement est **dans** l'archive du webtoon réel (titre, titres alternatifs, auteurs, description, état, tags, entrée de couverture).
- Recherche et filtres côté client (`web/src/library/filters.ts`).

### Identité des séries (lot 5.1, sans migration)

Avant de retirer les emplacements disparus d'une passe, une série S est **reprise** par la série N où ses tomes sont réapparus si : S n'a plus aucun autre fichier, ses tomes revus le sont tous dans la seule série N, N est née pendant cette passe et n'a que des tomes venus de S. S prend les fichiers, le dossier et le nom de N (N est supprimée) : même `series.id`, donc réglages, métadonnées et verrous conservés. Les tomes se reconnaissent par leur hash (principe 8), sans heuristique sur les noms.

Restent des séries distinctes : déplacement partiel, fusion dans une série existante, réunion de deux séries dans un nouveau dossier, éclatement en plusieurs dossiers, copie déjà connue dont l'original disparaît (elle garde ses propres réglages), renommage accompagné de l'ajout d'un nouveau tome dans la même passe (limite connue). Un renommage se reprend au scan complet de la bibliothèque : un scan limité au nouveau dossier crée une nouvelle série (limite connue).

### Base (une migration par lot qui s'en sert)

- **0008 (lot 5.2)** ✅ : `volume_metadata (volume_id, source, data)` : `ComicInfo.xml` et `index.json` normalisés (`shiori_core::LocalMetadata` en JSON), réécrits à chaque scan du tome. Les tomes déjà scannés sont relus grâce à `SCANNER_VERSION = 2` (pas de colonne `read_at`).
- **0009 (lot 5.3)** ✅ : `series_metadata (series_id, edited, merged)` : `edited` = champs édités (un champ présent est verrouillé, `null` compris), `merged` = vue fusionnée recalculée après chaque passe, chaque modification et à l'ouverture de la base ; `series.publication_status` en garde le statut (filtres). Le statut réglé à la main en phase 4 devient un champ édité. `volume_edits (volume_id, edited)` ; la vue d'un tome se calcule à la lecture.
- **0012 (lot 5.7b)** ✅ : `app_settings.metadata_adult` (fiches pour adultes, exclues par défaut).
- **0011 (lot 5.6)** ✅ : `covers (id, media_type, width, height, data, created_at)`.
- **0010 (lot 5.5b)** ✅ : `series_sources` (identifiant externe, `matched_by` auto / user, dernière fiche, candidats en JSON, `last_error` en code stable) ; `app_settings.metadata_online` (désactivé par défaut) et `metadata_sources` (ordre choisi, `NULL` = d'après la langue).

### Routes

| Méthode | Route | Rôle |
|---|---|---|
| GET | `/api/series/{id}` | bloc `metadata` : champs, `locked`, sources liées, dernière erreur par source |
| GET | `/api/series`, `/api/libraries/{id}/series` | titre affiché, auteurs, titres alternatifs (recherche), `cover_url` |
| PATCH | `/api/series/{id}` | tout champ éditable (le verrouille) ; `{"unlock": [...]}` ; source principale de la série |
| GET / PATCH | `/api/volumes/{id}` | métadonnées du tome, même verrouillage |
| GET | `/api/series/{id}/metadata/search?source=&q=` | candidats d'une source |
| PUT / DELETE | `/api/series/{id}/metadata/links/{source}` | lier un candidat (récupère et fusionne) / délier |
| POST | `/api/series/{id}/metadata/refresh` | actualiser les sources liées |
| GET | `/api/metadata/thumbnail?source=&id=` | miniature de candidat relayée (hôtes en liste blanche, taille plafonnée) : le navigateur ne contacte aucun CDN |
| GET | `/api/covers/{hash}?w=&f=` | couverture externe stockée, redimensionnée, URL immuable |
| GET / POST | `/api/admin/metadata`, `/api/admin/metadata/refresh` | état (à confirmer, erreurs, dernière tâche) ; tout identifier ou tout actualiser |
| PUT | `/api/settings` | `metadata_online`, `metadata_sources` |

### Lots (une branche + une PR chacun)

- ✅ **5.1 `feat/series-identity`** : reprise d'une série dont le dossier est renommé ; plan de la phase dans ROADMAP.md.
- ✅ **5.2 `feat/local-metadata`** (liens limités à `http(s)` au lot 5.3, après un vrai tome dont `<Web>` contenait un titre) : migration 0008 ; `shiori_core::metadata` lit `ComicInfo.xml` (quick-xml ; UTF-8 avec ou sans BOM, UTF-16 ; auteurs par rôle, genres, date partielle, `Manga` → proposition de sens) et `index.json` (titres, auteurs « a / b », description, état, tags, lien) à la racine, dans la même passe que les pages (RAR solid compris) ; fichier illisible ou de plus de 4 Mio = avertissement `metadata_invalid`, jamais une erreur de tome ; `SCANNER_VERSION = 2` pour relire les tomes connus. Vérifié sur le webtoon réel (son `index.json`) et deux vrais CBR (inchangés).
- ✅ **5.3 `feat/metadata-api`** : migration 0009 ; fusion (fonctions pures) : pour chaque champ, le premier `index.json` puis le premier `ComicInfo.xml` dans l'ordre des tomes, recouverts par les champs édités ; titre des fichiers avant le nom du dossier (Q34), autres titres et nom du dossier en titres alternatifs ; `PATCH /api/series/{id}` et `PATCH /api/volumes/{id}` génériques (champ donné = édité et verrouillé, `unlock: [...]`, champ inconnu ou mauvais type = 400) ; n'importe quelle page de la série comme couverture (oubliée si sa page disparaît) ; listes : titre affiché, auteurs, titres alternatifs, couverture choisie ; détails : `metadata` et `locked`. Vérifié sur une copie de la base d'essai : le webtoon réel donne auteurs, résumé, tag, statut et lien.
- ✅ **5.4 `feat/web-metadata-edit`** (UI) : page série (auteurs, repères, résumé repliable, genres et tags en puces, autres titres, liens `http(s)` seulement) ; fenêtre « Modifier » avec cadenas et « Rétablir » par champ (le sélecteur de statut de l'en-tête y passe, verrou compris) ; auteur sous le titre des cartes et sur la couverture de repli ; recherche par auteur et titre alternatif ; « Utiliser comme couverture » dans le panneau Affichage du lecteur ; notification du lecteur à sa place sur mobile (`:root:has([data-reader])`, plus de barre d'onglets) ; **test e2e « tome suivant » : cause trouvée et corrigée**, un vrai défaut du lecteur (l'écouteur clavier, réabonné après l'affichage, traitait une touche juste après « Fin » avec l'ancienne page ; `useEffectEvent`), 10 passages sur 10 en local. Reportés : la liste « Source principale » (lot 5.7, sans objet avant les sources web) et l'édition d'un tome dans l'interface (l'API existe, lot 5.3).
- ✅ **5.5a `feat/metadata-sources`** : crate `shiori-meta` : trait `Source`, clients AniList (GraphQL, romans exclus, résumé débarrassé du HTML, auteurs scénario/dessin d'après le rôle (« Original Creator » compte comme scénario), tags sans divulgâchage) et MangaDex (REST, titres et résumés par langue, auteur/dessinateur, genres et thèmes, couverture 512 px), normalisés dans `WebMetadata` ; débit minimal par source (AniList 2 s, MangaDex 250 ms), `Retry-After` respecté sans requête de plus (au-delà de 10 s d'attente on renonce), délai global 15 s, erreurs typées à code stable (`offline`, `rate_limited`, `not_found`, `http_error`, `bad_response`) ; identifiants validés avant de partir ; rapprochement automatique (Q30) en deux temps : titres principaux puis synonymes (la parodie « Wan Piece » a « One Piece » en synonyme chez AniList). Client HTTP **ureq 3** (rustls + ring, synchrone) plutôt que reqwest, dont la dorsale par défaut (aws-lc-rs) peut demander cmake sous Windows. Tests contre un faux serveur local (aucun appel réseau en CI) ; essai réel : « one piece » trouvé et lu chez les deux, rapproché tout seul.
- ✅ **5.5b `feat/online-sources`** : migration 0010 (`series_sources` : liée avec sa dernière fiche, cherchée avec ses candidats, ou en échec avec son code ; `app_settings.metadata_online`, `metadata_sources`) ; fusion avec le web après les fichiers et avant les éditions (`with_web` : titre Q34 — fichiers, puis le web dans la langue de l'interface, puis romaji, puis le dossier ; résumé des fichiers, puis dans la langue de l'interface, puis en anglais ; autres champs vides pris dans l'ordre des sources ; autres titres et liens additionnés), recalculée quand la langue ou l'ordre change ; routes `GET …/metadata/search`, `PUT`/`DELETE …/metadata/links/{source}`, `POST …/metadata/refresh`, `GET /api/admin/metadata`, `POST /api/admin/metadata/identify` ; identification en tâche de fond à la fin de chaque passe de scan (si le réseau est activé), **arrêtée net sur `rate_limited` ou `offline`** (aucune erreur notée série par série), une seule à la fois ; une série liée qui échoue à la relecture garde sa fiche ; les candidats n'exposent jamais l'adresse de couverture ; résumés en **texte brut** pour toutes les sources (Markdown de MangaDex retiré, décision du 2026-11-13 : cohérent avec AniList et les fichiers, sans moteur de rendu ni risque). Essai réel sur la base d'essai : 6 séries sur 13 liées toutes seules chez les deux sources (One Piece, Berserk, Kingdom avec un résumé français…), « Monster » et « Akira » à confirmer (homonymes), Yotsuba affiché sous son titre français.
- ✅ **5.6 `feat/external-covers`** : migration 0011 : table `covers` (couvertures téléchargées gardées **dans la base**, persistantes et sauvegardées avec elle, identifiées par le hash BLAKE3 de leur contenu ; JPEG, PNG ou WebP seulement) ; chaque source télécharge ses images en `https` depuis ses **seuls hôtes** (`*.anilist.co`, `uploads.mangadex.org`), sans identifiants ni port, **aucune redirection suivie**, 8 Mio au plus, 4 096 px de côté au plus (SSRF, image piégée) ; `GET /api/metadata/thumbnail/{source}/{id}` relaie la vignette d'un résultat, candidat ou fiche liée (gardée un jour par le navigateur) ; `POST /api/series/{id}/metadata/cover {source}` prend la couverture de la fiche liée (champ édité `cover_image_id`, exclusif avec `cover_page_id`, validé) ; `GET /api/covers/{id}?w=&f=` sert l'originale ou un rendu aux largeurs fixes, URL immuable, ETag/304 (rendu à la demande, sans cache disque : couvertures de 500 px au plus). Listes : `cover_image_id`. Essai réel : vignette AniList de Berserk relayée, couverture MangaDex de One Piece gardée et servie en 320 px.
- ✅ **5.7a `feat/web-metadata-admin`** (UI) : section « Métadonnées web » de l'administration : case d'activation avec l'explication de ce qui part (le titre seul, client générique), ordre des sources réglable aux boutons Monter/Descendre (pas de glisser-déposer, WCAG 2.2) et retour à l'ordre par défaut, « Identifier les séries » avec progression et raison d'un arrêt (`rate_limited`, `offline`), liste « À vérifier » menant à chaque série, **crédits AniList et MangaDex** ; couvertures des sources affichées sur les cartes et la page série (`cover_image_id`) ; codes d'erreur des sources traduits ; le client lit une réponse sans corps (202) sans erreur (défaut trouvé au lancement de l'identification).
- ✅ **5.7b `feat/web-metadata-match`** (UI) : bouton « Identifier » sur la page série (réseau activé) ; fenêtre façon Plex : liste déroulante de la source (celle qui a des fiches à confirmer d'abord, sinon celle de la langue), titre à chercher, **grille de vignettes relayées** des candidats (bouton entier, couverture de repli si la vignette manque), message avec une piste quand rien ne correspond, erreur traduite ; pour une source liée : comment elle l'a été, lien vers sa fiche, dernier échec, « Actualiser », « Utiliser sa couverture », « Délier » ; crédits. « Source principale » dans « Modifier » (sources liées seulement, `main_source` éditable côté serveur) : elle passe devant l'ordre réglé pour la série, et son titre et son résumé l'emportent sur la préférence de langue (les autres fiches en repli). Une source principale déliée cesse de l'être et ne pèse plus. **Fiches pour adultes exclues par défaut** des recherches et identifications (AniList `isAdult: false`, MangaDex `safe` + `suggestive`), case « Inclure les fiches pour adultes » dans l'administration (migration 0012 `app_settings.metadata_adult`) ; contrôlé en réel : sur « monster », 0 fiche pour adultes sur 10 avec le filtre, 4 sans. Défaut trouvé à ce contrôle et corrigé : `isAdult: null` vidait la recherche AniList, l'argument est omis quand les fiches pour adultes sont permises.
- ✅ **5.8 `chore/phase5`** : bilan, parcours de bout en bout des métadonnées (`e2e/metadata.spec.ts`), README.

### Dépendances prévues (dernière version vérifiée au lot)

`quick-xml` (serde) pour ComicInfo ; `reqwest` en rustls (pas d'OpenSSL) ; si le fournisseur par défaut (aws-lc-rs) exige cmake ou un compilateur C sous Windows, `ring`. Débit limité par un intervalle tokio par source, sans crate.

### Tests

- **Rust** : reprise des séries renommées et ses cas limites ; fusion et verrous en table (priorité, langue, source principale, verrou, déverrouillage) ; migrations sur une base de la phase précédente ; ComicInfo valide, invalide, BOM, très gros, absent, RAR solid ; `index.json` réel allégé en fixture ; sources contre un serveur axum local qui rejoue des réponses enregistrées (succès, 404, 429 + `Retry-After`, délai, hors ligne, JSON inattendu) ; aucune requête ne contient de chemin ou de nom de fichier ; relais des miniatures refusé hors liste blanche (SSRF) ; couvertures stockées et URLs immuables. Couverture ≥ 80 % lignes et branches.
- **Vitest** : fenêtre « Modifier » (cadenas, déverrouillage, erreurs traduites), grille d'identification, filtres par auteur et titre alternatif, catalogues. Seuils 80 %.
- **Bout en bout** : modifier un champ puis rescanner (valeur gardée), renommer un dossier (réglages et métadonnées gardés), ComicInfo affiché. Aucun appel réseau en CI.
- **Lots d'interface** : UI/UX Pro Max, MCP shadcn, captures aux quatre tailles (titres très longs, résumés vides, couvertures absentes).

### Bilan de la phase 5

- Livré :
  - **Métadonnées des fichiers** : `ComicInfo.xml` et `index.json` lus pendant le scan (RAR solid compris), normalisés, gardés par tome ; fichier illisible = avertissement, jamais une erreur de tome.
  - **Édition manuelle avec verrouillage par champ** : fenêtre « Modifier » (cadenas, « Rétablir »), fusion fichiers → web → éditions, titre selon Q34, liens `http(s)` seulement, n'importe quelle page comme couverture ; édition d'un tome par l'API.
  - **Séries qui survivent au renommage de leur dossier** (même id, réglages et métadonnées gardés).
  - **Sources web** (désactivées par défaut) : clients AniList et MangaDex (`shiori-meta`, ureq + rustls/ring), débit sous les limites, `Retry-After` respecté, rien de personnel envoyé ; identification automatique après chaque scan (lien seulement si une correspondance unique), fenêtre « Identifier » façon Plex, source principale par série, ordre des sources réglable (défaut selon la langue), fiches pour adultes exclues par défaut, crédits.
  - **Couvertures des sources** : téléchargées depuis les seuls hôtes connus (aucune redirection, SSRF), gardées en base, servies redimensionnées ; vignettes des candidats relayées par le serveur.
  - **Interface** : page série enrichie, auteur sur les cartes et la couverture de repli, recherche par auteur et titre alternatif, section « Métadonnées web » de l'administration.
- Validé : vrais fichiers (webtoon de 2 Go et son `index.json`, tome Shangri-la Frontier et son ComicInfo, deux vrais CBR) ; vraies sources (One Piece, Berserk, Kingdom… liés tout seuls, homonymes à confirmer, filtre adultes contrôlé) ; parcours de bout en bout (ComicInfo, modification, rescan, renommage, retour aux fichiers) ; captures aux quatre tailles à chaque lot d'interface.
- Qualité : 242 tests Rust, 170 tests Vitest, 9 tests de bout en bout ; 9 PR (#49 à #57), chacune passée en review Ponytail.
- Défauts trouvés et corrigés en route : `<Web>` d'un vrai ComicInfo pris pour des liens ; parodie « Wan Piece » qui bloquait le rapprochement de « One Piece » ; résumés MangaDex en Markdown ; série supprimée qui arrêtait l'identification ; redirections qui contournaient la liste blanche des hôtes ; image piégée trop grande ; **test e2e « tome suivant » : un vrai défaut du lecteur** (touche traitée avec l'ancienne page) ; notification du lecteur trop haute sur mobile ; réponse 202 sans corps mal lue ; source principale déliée encore active ; `isAdult: null` qui vidait la recherche AniList.
- Reste ouvert pour la suite :
  - édition d'un tome dans l'interface (l'API existe) ; titres de tome venus de ComicInfo non affichés dans la liste des tomes ;
  - MangaUpdates et Comic Vine ; actualisation périodique des fiches liées ;
  - `shortcut:` notés : couvertures abandonnées jamais purgées, rendus de couvertures sans cache disque, mémoire des vignettes de recherche bornée à 1 000 adresses, arrêt du serveur qui attend la requête d'identification en cours (≈ 25 s au pire) ;
  - des liens enregistrés au lot 5.2 peuvent contenir autre chose que des adresses (filtrés à l'affichage, corrigés au prochain rescan complet) ;
  - un passage local de la couverture Vitest a eu 5 échecs (machine chargée, probablement des délais), vert ensuite et en CI ;
  - de la phase 4 : registre shadcn et le paquet `cn`, journal d'un scan en cours non rafraîchi, essai sur la tablette réelle ; vérifier `:root:has()` (notification du lecteur) sur un vrai Safari iOS.

### Questions de la phase 5 (résolues le 2026-11-13)

- **Q28 Sources et priorité** ✅ : sources locales, AniList et MangaDex en phase 5 (MangaUpdates et Comic Vine plus tard). Priorité par champ : manuel (verrou) > fichiers locaux > sources web dans l'ordre choisi. L'ordre se règle dans Administration → Métadonnées ; par défaut il suit la langue de l'interface : anglais → AniList d'abord, français → MangaDex d'abord. Une série peut imposer sa source principale (liste déroulante dans « Modifier »).
- **Q29 Identité des séries** ✅ : reprise par le hash des tomes (ci-dessus).
- **Q30 Rapprochement** ✅ : automatique seulement si un seul candidat a exactement le même titre normalisé ; sinon « à confirmer ». Identification manuelle façon Plex : choisir la source, chercher, choisir parmi les couvertures des candidats.
- **Q31 Niveau** ✅ : séries pour toutes les sources ; tomes (titre, résumé, date, auteurs) depuis ComicInfo et l'édition manuelle ; numéro, ordre et pages restent au scan.
- **Q32 Couvertures** ✅ : page 0 du premier tome par défaut ; au choix, n'importe quelle page ou une couverture de source, stockée de façon persistante.
- **Q33 Réseau et vie privée** ✅ : désactivé par défaut, activé dans l'administration avec une explication de ce qui est envoyé ; agent générique ; débit sous les limites publiées ; hors ligne = état visible, jamais bloquant ; miniatures relayées. Conditions relues le 2026-11-13 (lot 5.5a) : AniList 90 requêtes/min, **30 en ce moment** (confirmé par `X-RateLimit-Limit`), 1 min de blocage au-delà avec `Retry-After`, ni collecte massive ni usage comme stockage, gratuit sous 150 $/mois de revenus ; MangaDex environ 5 requêtes/s par IP, agent honnête obligatoire, pas de lien direct vers ses images, **créditer MangaDex** (lot 5.7), pas de publicité.
- **Q34 Titre affiché** ✅ (précisé le 2026-11-13) : titre édité, sinon celui des fichiers (`index.json`, puis `ComicInfo.xml`), sinon celui des métadonnées web, sinon le nom du dossier. La langue ne sert qu'à choisir parmi les titres d'une source web : celui de la langue de l'interface s'il existe, sinon romaji. Les autres titres (nom du dossier compris) restent cherchables comme titres alternatifs.

## Phase 6 : distribution, documentation et thèmes (plan validé le 2026-11-13)

### Objectif

Rendre Shiori installable et maintenable par quelqu'un d'autre que son développeur : Dependabot, binaires Linux / Windows publiés à chaque version, image Docker, thèmes de l'interface (sombre, clair, OLED, sépia), PWA utilisable hors ligne, documentation (technique, installation, fonctionnelle), puis la version `v0.1.0`.

### Existant vérifié (2026-11-13)

- **Un seul binaire** : `shiori serve` embarque le front en release (`rust-embed`) ; base dans le dossier de données de l'OS (`SHIORI_DATA_DIR` ou `--db`), cache dans celui de l'OS (`SHIORI_CACHE_DIR` ou `--cache-dir`). Code pur Rust : SQLite embarqué (`sqlite-bundled`), TLS par ureq 3 + rustls avec certificats intégrés (aucune dépendance aux certificats de l'image). `--version` existe, version 0.1.0.
- **Arrêt** : `serve` ne s'arrête proprement que sur Ctrl-C ; `docker stop` envoie SIGTERM puis tue au bout de 10 s, même pendant un scan.
- **Écoute** : en écoute locale, seul un `Host` local est accepté (parade au DNS rebinding) ; en écoute réseau (`0.0.0.0`, obligatoire dans un conteneur), ni authentification ni cette parade.
- **CI** : jobs `backend` (linux, windows, macos), `coverage`, `web`, `e2e` ; pas de release ni de Dependabot.
- **Couleurs** : toutes passent par les 53 jetons de `web/src/index.css` (une seule série de valeurs, le sombre) ; une douzaine de couleurs en dur, presque toutes dans le lecteur ; `theme.test.ts` vérifie déjà que chaque jeton utilisé est défini.
- **PWA** : `manifest.webmanifest` minimal, zones sûres posées, pas de service worker ; pages servies immuables (`Cache-Control: immutable`, ETag).

### Décisions (Q35 à Q41, validées le 2026-11-13)

- **Q35 Écoute dans le conteneur** ✅ : option `--allowed-host` (variable `SHIORI_ALLOWED_HOSTS`, répétable) qui garde la parade au DNS rebinding même en écoute réseau ; le `compose.yaml` d'exemple publie `127.0.0.1:7878` ; la documentation explique le reverse proxy avec authentification (Caddy). L'authentification reste « Plus tard ».
- **Q36 Image Docker** ✅ : `distroless/static:nonroot` + binaire musl, amd64 et arm64, sur ghcr.io ; volumes `/data`, `/cache`, bibliothèques en lecture seule ; `user:` dans compose plutôt que `PUID`/`PGID` (pas de shell dans l'image).
- **Q37 Binaires** ✅ : Linux x86_64 et aarch64 (musl, statiques), Windows x86_64 ; macOS seulement s'il passe sans effort. Sur tag `v*`, `SHA256SUMS`, notes générées par GitHub (pas de CHANGELOG tenu à la main). Premier tag `v0.1.0` en fin de phase.
- **Q38 PWA hors ligne** ✅ : coquille de l'interface + pages déjà lues gardées en cache (immuables, donc sans risque) ; tomes téléchargés explicitement : plus tard. Service worker fait main (pas de `vite-plugin-pwa`), invitation à installer discrète.
- **Q39 Dependabot** ✅ : hebdomadaire, une PR groupée par écosystème pour mineures et correctifs, majeures à part, pas de fusion automatique ; nightly de couverture et `cargo-llvm-cov` restent figés et montés à la main.
- **Q40 Thèmes** ✅ : un système de thèmes plutôt qu'un seul mode clair. Un thème = un bloc `[data-theme="…"]` dans `index.css` qui redéfinit les jetons ; livrés : **sombre** (défaut actuel), **clair** (d'après la palette neutre claire de shadcn), **OLED** (noir pur), **sépia**. Réglage `theme` dans `/api/settings` : `system` (sombre ou clair selon `prefers-color-scheme`) ou un thème imposé, sélecteur dans l'administration à côté de la langue. Accent ambre dans tous les thèmes ; le lecteur reste sombre. Contrastes WCAG AA des couples importants mesurés par `theme.test.ts` pour chaque thème (un thème illisible casse le build). Thèmes créés par l'utilisateur : non.
- **Q41 Documentation** ✅ : dans `docs/` ; installation en français et en anglais, technique et fonctionnelle en français ; le README y renvoie.

### Lots (une branche + une PR chacun)

- ✅ **6.1 `ci/dependabot`** : `.github/dependabot.yml` (cargo, npm, actions) selon Q39 ; CLAUDE.md mis à jour.
- ⏳ **6.2 `ci/release`** : `release.yml` sur tag `v*` : build du front une fois, matrice des cibles (aarch64 musl sur runner `ubuntu-24.04-arm`, sans compilation croisée), archives, `SHA256SUMS`, `gh release create --generate-notes`. Essai sur `v0.1.0-rc.1`.
- ⏳ **6.3 `feat/docker`** : arrêt sur SIGTERM (Unix) ; `--allowed-host` (Q35) ; `Dockerfile` à partir des binaires du lot 6.2, `compose.yaml`, image multi-architecture poussée depuis `release.yml`. Tests : hôte autorisé / refusé, arrêt sur SIGTERM, `docker run` réel.
- ⏳ **6.4 `feat/web-themes`** : jetons des quatre thèmes, couleurs en dur hors lecteur remplacées par des jetons, réglage `theme` (Rust + API), sélecteur avec aperçu, `theme-color` suivant le thème, lecteur forcé en sombre, contrastes testés, DESIGN_SYSTEM.md mis à jour. Captures Playwright aux quatre tailles dans chaque thème.
- ⏳ **6.5 `feat/web-pwa`** : service worker (coquille + images de pages déjà lues ; jamais le reste de `/api`), invitation à installer, état hors ligne. Test e2e : réseau coupé, une page déjà vue se relit.
- ⏳ **6.6 `fix/phase6-leftovers`** : journal d'un scan en cours rafraîchi, titres de tome venus de ComicInfo affichés, purge des couvertures abandonnées.
- ⏳ **6.7 `docs/final`** : `docs/installation.fr.md`, `docs/installation.en.md`, `docs/technique.md`, `docs/fonctionnel.md` (codes de diagnostic, rangement conseillé), rédigés à partir du graphe Graphify, puis `/graphify . --update`. Relecture : suivre l'installation à la lettre avec l'image et le binaire publiés.
- ⏳ **6.8 `chore/phase6-review`** : bilan de la phase 6, tag `v0.1.0`, release et image vérifiées de bout en bout.

### Dépendances

Aucune nouvelle prévue (service worker fait main, `tokio::signal::unix` déjà disponible) ; dernière version vérifiée au lot si le besoin apparaît.

### Risques

- Compilation musl aarch64 de `ring` et `ravif` non encore essayée (lot 6.2).
- L'image Docker reste sans authentification : Q35 réduit le risque sans l'éliminer, la documentation doit le dire clairement.

### Reste ouvert des phases précédentes (hors phase 6)

Édition d'un tome dans l'interface, MangaUpdates et Comic Vine, actualisation périodique des fiches, autres `shortcut:` notés au bilan de la phase 5, `:root:has()` sur un vrai Safari iOS, essai sur la tablette réelle, registre shadcn et le paquet `cn`.

## Fonctionnalités ajoutées en cours de route

### Menu d'administration des scans (demandé le 2026-11-11)

Objectif : piloter les scans depuis l'application et consulter leurs journaux, sans terminal.

- **Phase 2 (API, lots 2.6 et 2.7)** : lancer un scan (une bibliothèque, un dossier, un tome ; options `deep` et `rehash`), voir le scan en cours (progression, fichier courant), l'annuler ; historique des passes (`scan_runs` : début, fin, options, durée, compteurs ok / avertissement / erreur / inchangés) ; journal détaillé d'une passe (fichiers scannés, problèmes, erreurs fatales). À prévoir : une table de journal par passe (horodatage, niveau, fichier, message) et une durée de conservation configurable.
- **Phase 4 (UI)** : page « Administration » → « Scans » : bouton « Scanner maintenant », passe en cours avec progression, historique filtrable, détail d'une passe avec ses journaux et un lien vers chaque tome en erreur. Accès réservé aux administrateurs quand les rôles arriveront (phase « Plus tard »). Lots 4.7 et 4.8.

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
- 2026-11-12 : interface bilingue français / anglais (lot 3.4b) ; langue enregistrée côté serveur, traduction par l'interface d'après les codes stables de l'API.
- 2026-11-12 : plan de la phase 4 validé : breakpoint `lg` (Q18), statut de publication manuel (Q23), « Continuer la lecture » avec tome suivant (Q24), chemin en champ texte (Q25), statistiques reportées (Q26), tri « Récemment ajoutés » (Q27).
- 2026-11-13 : phase 4 terminée (bibliothèque façon Plex, navigation définitive, administration des bibliothèques et des scans, DESIGN_SYSTEM.md) ; l'accent de l'interface reste ambre, les couleurs du logo lui sont réservées.
- 2026-11-13 : mode clair de l'interface reporté en phase 6 ; plan de la phase 5 préparé dans une nouvelle session (notes de passation dans « Phase 5 »).
- 2026-11-13 : plan de la phase 5 validé (Q28 à Q34) : sources locales + AniList + MangaDex, ordre réglable (par défaut selon la langue de l'interface), source principale par série, identification façon Plex, réseau désactivé par défaut ; une série renommée garde son identité (lot 5.1).
- 2026-11-13 : résumés des sources web en texte brut (le Markdown de MangaDex est retiré, pas de moteur de rendu dans l'interface).
- 2026-11-13 : fiches pour adultes exclues par défaut des recherches web, une case de l'administration les permet.
- 2026-11-13 : phase 5 terminée (métadonnées des fichiers et du web, édition manuelle avec verrouillage par champ, séries qui survivent au renommage, identification façon Plex, couvertures des sources).
- 2026-11-13 : plan de la phase 6 préparé dans une nouvelle session (notes de passation dans « Phase 6 »).
- 2026-11-13 : plan de la phase 6 validé (Q35 à Q41) : Dependabot, release sur tag, image Docker distroless avec `--allowed-host`, système de thèmes (sombre, clair, OLED, sépia) au lieu du seul mode clair, PWA hors ligne limitée aux pages déjà lues, documentation dans `docs/`.
