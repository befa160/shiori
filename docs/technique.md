# Documentation technique

Comment Shiori est construit, pour qui veut le maintenir ou y contribuer. Pour l'utiliser : [documentation fonctionnelle](fonctionnel.md) ; pour l'installer : [installation](installation.fr.md). Les décisions et leur historique sont dans [ROADMAP.md](../ROADMAP.md), les règles de travail dans [CLAUDE.md](../CLAUDE.md), l'interface dans [DESIGN_SYSTEM.md](../DESIGN_SYSTEM.md).

## Vue d'ensemble

Un seul binaire Rust, `shiori`, avec deux commandes :

- `shiori scan <chemins>` : scan d'intégrité en ligne de commande, avec ou sans base ;
- `shiori serve` (ou `shiori` sans argument) : serveur HTTP qui sert l'API JSON sous `/api` et l'application web (React) intégrée au binaire.

```
fichiers CBZ / CBR ──scan──▶ SQLite ──API JSON / images──▶ application web (PWA)
   (jamais modifiés)          (vérité)    (axum)                (React, service worker)
```

Le scan est la seule source de vérité sur les pages : l'API et le lecteur ne font que relire ce qu'il a établi.

## Principes et invariants

Les règles non négociables sont dans [CLAUDE.md](../CLAUDE.md) (« Principes d'architecture ») ; voici ce qu'elles impliquent dans le code.

- **Identité d'un tome** : BLAKE3 du contenu complet du fichier (`VolumeId`, `shiori-core/src/id.rs`). Renommer ou déplacer ne change rien ; modifier le contenu crée un nouveau tome. Hash complet au premier passage, puis taille + date de modification pour détecter un changement (`--rehash` force la relecture).
- **Identité d'une page** : `BLAKE3(volume_id ‖ 0x00 ‖ nom brut de l'entrée)`, tronqué à 128 bits (`PageId`), sur les octets bruts du nom dans l'archive (aucun décodage CP437 / UTF-8 ne peut changer un identifiant). Les deux formules sont figées par des tests `*_is_frozen` : les changer invaliderait toutes les adresses et toutes les progressions.
- **Ordre des pages** : tri naturel des noms d'entrées (`alphanumeric-sort`), stocké en `ordinal` ; à valeur égale, `9` précède `09`. L'ordre dans l'archive ne compte jamais : le lecteur reçoit la liste ordonnée des `page_id`, et le serveur lit chaque page par l'index enregistré pour cet identifiant (test « invariant de lecture » de bout en bout).
- **Adresses d'images immuables** : `/api/pages/{page_id}/image` répond avec `Cache-Control: immutable` et un ETag ; une page ne change jamais sous un même identifiant, donc aucun cache (navigateur, service worker, disque) n'a besoin d'être invalidé.
- **Erreur visible** : une page absente ou illisible renvoie une erreur JSON avec un code stable ; l'interface affiche cette erreur à la place de la page, jamais une autre image. Le service worker ne garde que les réponses 200.
- **Fichiers sources** : ouverts en lecture seule, jamais modifiés ni extraits en entier (sauf le cache borné des RAR solides).
- **Portabilité** : Rust pur autant que possible (SQLite embarqué, `ring` pour TLS), aucune commande externe, chemins indépendants du séparateur ; CI sous Linux, Windows et macOS.

## Le dépôt

```
backend/                   workspace cargo (toolchain stable, rust-toolchain.toml)
  crates/shiori-core/      archives, identités, parseur de noms, scan d'intégrité, ComicInfo / index.json
                           (sans base ni async)
  crates/shiori-db/        base SQLite (sqlx) : scans, bibliothèques, séries, progression, réglages,
                           métadonnées et leur fusion ; migrations dans migrations/
  crates/shiori-meta/      clients AniList et MangaDex (ureq + rustls), débit limité
  crates/shiori-testkit/   fixtures générées par du code (archives, images, cas tordus)
  crates/shiori/           le binaire : CLI de scan, serveur axum, job de scan, rendu des images,
                           cache disque, métadonnées web, front embarqué
web/                       React 19 + TypeScript + Vite + Tailwind + shadcn/ui
  src/api/                 client et types de l'API, requêtes TanStack Query
  src/reader/              lecteur (pages, doubles pages, webtoon virtualisé, gestes, progression)
  src/library/ src/browse/ accueil, grilles, page série, modification, identification
  src/admin/               bibliothèques, scans et journaux, métadonnées web, réglages
  src/offline/             tomes téléchargés et progression hors ligne
  src/i18n/                catalogues fr.ts (source) et en.ts (typé sur le français)
  public/sw.js             service worker
  e2e/                     tests Playwright contre un vrai `shiori serve`
docs/                      cette documentation et la note CBR
```

## Archives

- **CBZ** (`zip`) : lecture d'une entrée par l'annuaire central, sans extraction complète ; zip64 et gros fichiers gérés.
- **CBR** (`rars`, Rust pur, jamais `unrar`) : RAR 4 et 5. Non solide : lecture directe par index. Solide (pas d'accès aléatoire) : extraction une fois dans le cache disque (`extract/<tome>/<index>`), sous le même plafond que les rendus. Détail dans [cbr.md](cbr.md).
- **Scan d'intégrité** (`shiori-core::integrity`) : CRC et taille de chaque entrée, en-têtes et dimensions des images, doublons, fichiers parasites, sous-dossiers, ordre, archives tronquées ou chiffrées ; `--deep` décode chaque image. Chaque problème a un code stable et une gravité (liste et explications : [codes de diagnostic](fonctionnel.md#codes-de-diagnostic)). Une page illisible est marquée `broken`, le tome reste lisible pour le reste.
- **Métadonnées des fichiers** : `ComicInfo.xml` et `index.json` lus pendant le scan (RAR solide compris), normalisés, gardés par tome ; un fichier illisible est un avertissement, jamais une erreur de tome.

## Parseur de noms

`shiori-core::name::parse(chemin relatif)` est une fonction pure : série = premier dossier sous la racine, puis type (tome, chapitre, spécial, inconnu), numéro et fin d'intervalle d'après des marqueurs français, anglais et japonais. Règles et exemples : [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers). `PARSER_VERSION`, figée par un test, relance l'analyse des emplacements (sans rehash) quand elle change.

## Base de données

SQLite (`sqlx`, `sqlite-bundled`), un fichier `shiori.db`, migrations appliquées au lancement (`backend/crates/shiori-db/migrations/`, une par lot qui en a besoin). Tables :

| Table | Contenu |
|---|---|
| `volumes` | un tome par contenu : hash, taille, format, statut, nombre de pages, date du scan |
| `pages` | une ligne par page : `page_id`, tome, nom d'entrée, `ordinal`, index dans l'archive, largeur, hauteur, type, statut |
| `file_locations` | où se trouve chaque tome : chemin, taille, date, série, résultat du parseur de noms |
| `libraries`, `series` | bibliothèques (racine, type) et séries (dossier, réglages de lecture propres) |
| `scan_runs`, `scan_issues`, `scan_run_logs` | passes de scan, problèmes par tome, journal de chaque passe (conservé `--scan-log-days`) |
| `reading_progress` | page lue par tome (une seule progression tant qu'il n'y a pas de comptes) |
| `app_settings` | réglages globaux : langue, thème, sens des pages, métadonnées web |
| `volume_metadata`, `volume_edits` | métadonnées des fichiers par tome, et champs édités à la main |
| `series_metadata` | champs édités d'une série (`edited`) et vue fusionnée (`merged`) |
| `series_sources` | fiches web liées ou candidates par série et par source |
| `covers` | couvertures des sources choisies, gardées en base (BLAKE3 du contenu) ; celles qu'aucune série ne référence sont purgées au démarrage, après une heure de grâce |

Une série garde son identité quand son dossier est renommé : elle est retrouvée par le hash de ses tomes.

## API

JSON sous `/api`, erreurs `{ "code", "message" }` avec un code stable que l'interface traduit. Pas d'authentification (phase 7).

| Route | Rôle |
|---|---|
| `GET /api/health` | état et version |
| `GET, PUT /api/settings` | réglages (mise à jour partielle) |
| `GET, POST /api/libraries`, `DELETE /api/libraries/{id}` | bibliothèques (supprimer ne touche pas aux fichiers) |
| `GET /api/libraries/{id}/series`, `GET /api/series` | séries d'une bibliothèque, toutes les séries (filtres côté client) |
| `GET, PATCH /api/series/{id}` | détail d'une série (tomes, progression, métadonnées), modification de ses métadonnées |
| `PATCH /api/series/{id}/settings` | réglages de lecture propres à la série |
| `GET /api/continue` | « Continuer la lecture » |
| `GET /api/series/{id}/metadata/search` | recherche dans une source web |
| `PUT, DELETE /api/series/{id}/metadata/links/{source}` | lier ou délier une fiche |
| `POST /api/series/{id}/metadata/refresh`, `.../metadata/cover` | relire les fiches liées, prendre la couverture d'une source |
| `GET /api/metadata/thumbnail/{source}/{id}` | vignette d'un candidat, relayée par le serveur |
| `GET /api/covers/{id}` | couverture gardée, originale ou redimensionnée |
| `GET, PATCH /api/volumes/{id}` | détail d'un tome (emplacements, problèmes, réglages effectifs, progression), modification de ses métadonnées |
| `GET, PUT, DELETE /api/volumes/{id}/progress` | page lue |
| `GET /api/volumes/{id}/pages` | pages dans l'ordre de lecture |
| `GET /api/pages/{id}/image` | une page : originale, ou `?w=<largeur>&f=avif\|jpeg` |
| `GET, POST /api/admin/scans`, `GET /api/admin/scans/{id}`, `.../logs` | historique, lancer un scan, une passe, son journal |
| `GET /api/admin/scans/current`, `POST .../current/cancel` | scan en cours, annulation |
| `GET /api/admin/metadata`, `POST /api/admin/metadata/identify` | état des sources, identification de toutes les séries |
| `GET /api/admin/storage` | chemins complets de la base et du cache |

Tout le reste est l'application web : un fichier de `web/dist`, ou `index.html` pour une route de l'application (React Router). En debug, `web/dist` est lu sur le disque ; en release, il est intégré au binaire (`rust-embed`) : `npm run build` avant `cargo build --release`.

## Images

- **Largeurs fixes** (`render.rs`, `WIDTHS` : 160 à 2048 px) : le client demande la plus petite qui couvre sa largeur affichée en pixels réels ; une liste fermée borne le cache et empêche de faire calculer des milliers de variantes. Jamais d'agrandissement : une page plus étroite est servie telle quelle.
- **Formats** : AVIF pour les pages (`ravif`, qualité 70), JPEG pour les miniatures et les couvertures (qualité 85), redimensionnement Lanczos3, décodage plafonné en mémoire ; tout en Rust pur. Une page déjà en AVIF est servie telle quelle.
- **Cache disque** (`cache.rs`) indexé par (page, largeur, format, version du rendu) ; rien n'est jamais invalidé, le moins récemment lu part quand le plafond (`--cache-max-mib`, 2 Gio par défaut) est atteint. Au plus 4 rendus simultanés.
- **Webtoon** : les bandes sont servies en originale tant qu'elles ne dépassent pas trois fois la largeur utile (le navigateur les réduit sans coût serveur).

## Métadonnées

Fusion **champ par champ** (`shiori-db::metadata`) : champ édité (verrouillé) > fichiers (`index.json`, puis `ComicInfo.xml`) > sources web dans l'ordre choisi. La vue fusionnée d'une série est recalculée et stockée (`merged`) à chaque changement ; celle d'un tome est calculée à la lecture (pour toute une série en deux requêtes).

Sources web (`shiori-meta`, désactivées par défaut) : AniList (GraphQL) et MangaDex (REST), appelées hors du runtime async, débit sous les limites publiées, `Retry-After` respecté, nom de client générique, seul le titre de la série envoyé. Identification automatique après chaque scan : un lien seulement si un seul candidat a exactement le même titre normalisé. Couvertures téléchargées depuis les seuls hôtes connus de chaque source, sans suivre de redirection (parade SSRF), 4 096 px de côté au plus.

## Serveur

- **axum** + **tokio**, compression brotli / gzip des réponses texte, journaux `tracing` (`-v`, `-vv`, ou `RUST_LOG`).
- **Job de scan** (`job.rs`) : une passe à la fois (bibliothèque, dossier ou fichier), fichiers scannés en parallèle (`--jobs`), progression en mémoire, annulation entre deux fichiers ; une passe interrompue par un arrêt brutal est close au lancement suivant.
- **Arrêt propre** sur Ctrl-C et SIGTERM (`docker stop`) : le scan en cours est annulé et sa passe close.
- **Parade au DNS rebinding** : en écoute locale, seul un `Host` local est accepté ; `--allowed-host` / `SHIORI_ALLOWED_HOSTS` ajoute des noms et active ce contrôle en écoute réseau (l'image Docker le fait pour `localhost`).
- **Dossiers** : données et cache dans les dossiers de l'OS (crate `directories`), ou `SHIORI_DATA_DIR`, `SHIORI_CACHE_DIR`, `--db`, `--cache-dir`.

## Application web

- **React 19**, TypeScript, Vite, Tailwind, composants shadcn/ui (Radix), TanStack Query pour les données, React Router.
- **i18n** : `react-i18next`, aucun texte en dur ; `fr.ts` est la source, `en.ts` est typé dessus (une clé manquante casse le build). Erreurs de l'API et codes de scan traduits d'après leur code stable.
- **Thèmes** : jetons CSS au format shadcn dans `index.css`, un bloc `[data-theme]` par thème (sombre, clair, noir OLED, sépia), appliqués par `src/lib/theme.ts` ; contrastes mesurés par `theme.test.ts`. Règles : [DESIGN_SYSTEM.md](../DESIGN_SYSTEM.md).
- **Lecteur** : vues calculées à partir des dimensions du scan (doubles pages, moitiés), préchargement des deux vues suivantes et de la précédente (mesuré sur 4G lente), webtoon virtualisé (`@tanstack/react-virtual`) avec la place de chaque bande réservée, progression enregistrée 1 s après le dernier changement et à la fermeture.
- **PWA** : manifeste demandé avec les identifiants (reverse proxy avec mot de passe) ; service worker fait main (`public/sw.js`) : pages et couvertures en cache borné, fichiers `assets/` gardés, navigations jamais interceptées en ligne (le navigateur garde la main sur l'authentification du proxy), dernière interface servie hors ligne, reste de l'API toujours demandé au serveur.
- **Hors ligne** (`src/offline/`) : un tome téléchargé = les réponses JSON que lit le lecteur et chaque page à la taille de l'écran en portrait, dans le cache `shiori-offline-v1`, jamais vidé seul ; le service worker les sert sans réseau (une autre taille de la même page au besoin). Page par page : aucune requête au-delà de quelques Mo. Progression hors ligne gardée sur l'appareil et envoyée au retour du réseau.

## Tests et qualité

- **Rust** : tests unitaires dans des fichiers voisins (`src/foo/tests.rs`), tests d'intégration dans `tests/` (vrai binaire, vrai serveur TCP), fixtures générées par `shiori-testkit` ; `cargo clippy -D warnings`, `cargo fmt`.
- **Web** : Vitest + Testing Library (jsdom), oxlint, vérification de types au build.
- **Bout en bout** : Playwright dans Chromium contre un `shiori serve` de debug sur une bibliothèque générée : invariant de lecture page par page dans chaque mode, reprise, tome suivant, parcours complet, métadonnées, PWA et hors-ligne, captures aux quatre tailles.
- **Couverture** : au moins 80 % des lignes et des branches, Rust (`cargo-llvm-cov`, nightly figé) et web (Vitest v8), mesurée sur le code de production ; la CI échoue en dessous.

## CI, versions et distribution

- **CI** (`.github/workflows/ci.yml`) : format, clippy et tests sous Linux, Windows et macOS ; couverture ; build et tests du front ; bout en bout.
- **Release** (`release.yml`, sur un tag `v*`) : front construit une fois, binaires Linux x86_64 et aarch64 (musl, statiques, sur runners natifs), Windows x86_64, macOS aarch64 (non signé) ; chaque binaire est lancé et sa version comparée au tag ; archives et `SHA256SUMS` ; image Docker `distroless/static:nonroot` amd64 + arm64 essayée (interface, hôte refusé, arrêt propre) puis poussée sur `ghcr.io/befa160/shiori`. Un tag à suffixe (`v0.1.0-rc.1`) est une préversion. Une release créée dans l'interface de GitHub reçoit les fichiers sans perdre ses notes.
- **Dépendances** : toujours les dernières versions stables ; Dependabot ouvre chaque semaine une PR groupée par écosystème (cargo, npm, actions), une par version majeure.

## Choix de dépendances

| Besoin | Choix | Pourquoi |
|---|---|---|
| Serveur HTTP | axum, tokio, tower-http | standard de fait en Rust, léger, compression intégrée |
| Base | SQLite via sqlx (`sqlite-bundled`) | un fichier, rien à installer, requêtes vérifiées, migrations |
| ZIP | zip | zip64, accès direct par l'annuaire central |
| RAR | rars | Rust pur, RAR 4 et 5, solide compris (jamais `unrar`, non libre) |
| Hash | blake3 (`pure`) | rapide, sans code C |
| Images | image, ravif, imagesize | décodage et encodage AVIF / JPEG en Rust pur, dimensions sans décoder |
| HTTP sortant | ureq + rustls (`ring`) | client synchrone simple, TLS sans OpenSSL |
| Front embarqué | rust-embed | un seul binaire |
| Interface | React, Tailwind, shadcn/ui, TanStack Query | composants accessibles (Radix), état serveur simple |
