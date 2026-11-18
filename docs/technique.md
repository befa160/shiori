# Documentation technique

Comment Shiori est construit, pour qui veut le maintenir ou y contribuer. Pour l'utiliser : [documentation fonctionnelle](fonctionnel.md) ; pour l'installer : [installation](installation.fr.md). Les décisions et leur historique sont dans [ROADMAP.md](../ROADMAP.md), les règles de travail dans [CLAUDE.md](../CLAUDE.md), l'interface dans [DESIGN_SYSTEM.md](../DESIGN_SYSTEM.md).

## Vue d'ensemble

Un seul binaire Rust, `shiori`, avec deux commandes :

- `shiori scan <chemins>` : scan d'intégrité en ligne de commande, avec ou sans base ;
- `shiori serve` (ou `shiori` sans argument) : serveur HTTP qui sert l'API JSON sous `/api` et l'application web (React) intégrée au binaire.

```
fichiers CBZ / CBR / PDF ──scan──▶ SQLite ──API JSON / images──▶ application web (PWA)
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
  crates/shiori-core/      archives (CBZ, CBR, PDF), identités, parseur de noms, scan d'intégrité,
                           ComicInfo / index.json / Info des PDF
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
- **PDF** (`hayro-interpret`, Rust pur), lu comme une archive, sans conversion : Shiori sert les **images incluses**, pas la page PDF avec ses marges. Un `Device` qui ne dessine rien interprète chaque page (matrices, formulaires, `/Rotate`, couleurs, transparences) et relève ses images et son texte. Une page dont les images couvrent au moins la moitié de la surface donne une entrée `NNNN.jpg` ou `NNNN.png` (numéro de la page PDF) ; les autres sont écartées avec `pdf_page_without_image`. Un JPEG droit, gris ou RVB, sans masque, est servi **tel quel** : sa position dans le fichier est relevée à l'ouverture, puis lue directement (CRC revérifié) ; l'archive ouverte ne garde pas le contenu du PDF. Toute autre page est décodée puis encodée en PNG : image redressée, transparence gardée jusqu'à la superposition, page faite de plusieurs images (bandes, tuiles, calques) **recomposée** à la densité de l'image la plus fine et rognée à l'union des images. Garde-fous : `%%EOF` exigé (sinon `archive_truncated`, hayro rouvrirait un fichier coupé avec des pages en moins), 128 Mpx au plus par page, morceaux de moins d'un millième de la page écartés. Le scan charge le PDF une fois pour toutes ses pages.
- **Scan d'intégrité** (`shiori-core::integrity`) : CRC et taille de chaque entrée, en-têtes et dimensions des images, doublons, fichiers parasites, sous-dossiers, ordre, archives tronquées ou chiffrées ; `--deep` décode chaque image. Chaque problème a un code stable et une gravité (liste et explications : [codes de diagnostic](fonctionnel.md#codes-de-diagnostic)). Une page illisible est marquée `broken`, le tome reste lisible pour le reste.
- **Métadonnées des fichiers** : `ComicInfo.xml` et `index.json` lus pendant le scan (RAR solide compris), et le dictionnaire `Info` d'un PDF (auteurs, sujet → résumé, mots-clés → tags ; ni titre ni date, souvent ceux de l'outil de conversion), normalisés, gardés par tome ; un fichier illisible est un avertissement, jamais une erreur de tome.

## Parseur de noms

`shiori-core::name::parse(chemin relatif)` est une fonction pure : série = premier dossier sous la racine, puis type (tome, chapitre, spécial, inconnu), numéro et fin d'intervalle d'après des marqueurs français, anglais et japonais. Règles et exemples : [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers). `PARSER_VERSION`, figée par un test, relance l'analyse des emplacements (sans rehash) quand elle change.

## Base de données

SQLite (`sqlx`, `sqlite-bundled`), un fichier `shiori.db`, migrations appliquées au lancement (`backend/crates/shiori-db/migrations/`, une par lot qui en a besoin). Tables :

| Table | Contenu |
|---|---|
| `volumes` | un tome par contenu : hash, taille, format, statut, nombre de pages, date du scan |
| `pages` | une ligne par page : `page_id`, tome, nom d'entrée, `ordinal`, index dans l'archive, largeur, hauteur, type, statut |
| `file_locations` | où se trouve chaque tome : chemin, taille, date, série, résultat du parseur de noms |
| `libraries`, `series` | bibliothèques (racine, type) et séries (dossier, nom, statut) |
| `scan_runs`, `scan_issues`, `scan_run_logs` | passes de scan, problèmes par tome, journal de chaque passe (conservé `--scan-log-days`) |
| `users`, `sessions`, `user_libraries` | comptes (Argon2id, rôle), sessions (hash BLAKE3 du jeton, 90 jours glissants), bibliothèques ouvertes à un lecteur |
| `user_totp`, `user_recovery_codes` | secret TOTP d'un compte (dernier pas accepté, pour qu'un code ne resserve pas), codes de secours (hash BLAKE3, usage unique) |
| `api_tokens` | jetons d'API (`shiori_t_` + 256 bits, seul le hash BLAKE3 gardé, préfixe pour les reconnaître, expiration facultative) |
| `reading_progress` | page lue par compte et par tome |
| `user_series_settings`, `user_settings` | réglages de lecture d'une série et préférences (langue, thème, sens des pages), par compte |
| `legacy_progress`, `legacy_series_settings` | progression et réglages d'avant les comptes, repris par le premier administrateur |
| `app_settings` | réglages de l'instance : langue, thème et sens des pages par défaut, métadonnées web |
| `volume_metadata`, `volume_edits` | métadonnées des fichiers par tome, et champs édités à la main |
| `series_metadata` | champs édités d'une série (`edited`) et vue fusionnée, en français (`merged`) et en anglais (`merged_en`) |
| `series_sources` | fiches web liées ou candidates par série et par source |
| `covers` | couvertures des sources choisies, gardées en base (BLAKE3 du contenu) ; celles qu'aucune série ne référence sont purgées au démarrage, après une heure de grâce |

Une série garde son identité quand son dossier est renommé : elle est retrouvée par le hash de ses tomes.

**Sauvegardes** (`shiori-db::backup`, `shiori::backup`) : `VACUUM INTO` (copie cohérente, Shiori allumé) dans un fichier temporaire caché, renommé une fois complet, droits 0600 sous Unix, nom `<préfixe>-AAAAMMJJ-HHMMSS.db` (UTC, suffixé plutôt qu'écrasé). Une base existante qui va être migrée est d'abord copiée (`shiori-avant-<migration>-…`), quelle que soit la commande. La tâche périodique compte l'échéance depuis la plus récente `shiori-<date>.db` et n'efface qu'elles au-delà de `--backup-keep`. **Restauration** Shiori allumé : copie de travail vérifiée (`integrity_check`, table `_sqlx_migrations` présente, sinon une base SQLite quelconque serait migrée vide), migrée au schéma courant (une version plus récente est refusée), schémas comparés, puis la copie est attachée et chaque table vidée et remplie en une transaction (`defer_foreign_keys`) ; les sessions sont effacées, les jetons d'API reviennent à leur état d'alors.

## API

JSON sous `/api`, erreurs `{ "code", "message" }` avec un code stable que l'interface traduit. Session exigée sauf `/api/health` et `/api/auth/*` (cookie `shiori_session` ou `Authorization: Bearer`, avec le jeton de session ou un jeton d'API) ; un jeton d'API n'ouvre que les `GET` hors `/api/me` et `/api/admin`, et `PUT /api/volumes/{id}/progress` (403 `token_forbidden` sinon) ; progression, réglages de lecture et langue des titres sont ceux du compte connecté. Un lecteur ne voit que les bibliothèques qui lui sont ouvertes (404 pour le reste) ; les routes marquées **admin**, et toutes celles sous `/api/admin`, répondent 403 `forbidden` à un lecteur. Avec un mot de passe provisoire, seules `/api/me` et `/api/me/password` répondent (403 `password_change_required`).

| Route | Rôle |
|---|---|
| `GET /api/health` | état et version |
| `GET /api/auth/state`, `POST /api/auth/setup`, `.../login`, `.../totp`, `.../logout` | installation (premier administrateur, jeton affiché au démarrage), connexion (avec la double authentification, un défi de 5 minutes puis le code ou un code de secours), déconnexion |
| `GET, PUT /api/me`, `POST /api/me/password` | compte connecté et ses préférences, changer son mot de passe (ferme les autres appareils) |
| `GET /api/me/sessions`, `DELETE /api/me/sessions/{id}` | appareils connectés, en déconnecter un |
| `POST, DELETE /api/me/totp`, `POST /api/me/totp/confirm` | double authentification : commencer l'activation (mot de passe exigé, secret et URI `otpauth`), confirmer par un premier code (codes de secours rendus une fois), désactiver (mot de passe et code) |
| `GET, POST /api/me/tokens`, `DELETE /api/me/tokens/{id}` | jetons d'API du compte (le jeton en clair rendu une seule fois) |
| `GET, POST /api/admin/users`, `PATCH, DELETE .../{id}`, `POST .../{id}/password`, `DELETE .../{id}/sessions`, `DELETE .../{id}/totp` | **admin** : comptes (mot de passe provisoire montré une fois), rôle, bibliothèques, désactivation, nouveau mot de passe, déconnexion de tous ses appareils et jetons, retrait de la double authentification ; le dernier administrateur actif reste |
| `GET, PUT /api/settings` | réglages de l'instance (mise à jour partielle, **admin**) |
| `GET, POST /api/libraries`, `DELETE /api/libraries/{id}` | bibliothèques visibles ; créer (**admin**, `share` l'ouvre aux lecteurs existants) ; supprimer (**admin**, ne touche pas aux fichiers) |
| `GET /api/libraries/{id}/series`, `GET /api/series` | séries d'une bibliothèque, toutes les séries (filtres côté client) |
| `GET, PATCH /api/series/{id}` | détail d'une série (tomes, progression, métadonnées), modification de ses métadonnées (**admin**) |
| `PATCH /api/series/{id}/settings` | réglages de lecture de la série pour le compte |
| `GET /api/continue` | « Continuer la lecture » |
| `GET /api/series/{id}/metadata/search` | **admin** : recherche dans une source web |
| `PUT, DELETE /api/series/{id}/metadata/links/{source}` | **admin** : lier ou délier une fiche |
| `POST /api/series/{id}/metadata/refresh`, `.../metadata/cover` | **admin** : relire les fiches liées, prendre la couverture d'une source |
| `GET /api/metadata/thumbnail/{source}/{id}` | **admin** : vignette d'un candidat, relayée par le serveur |
| `GET /api/covers/{id}` | couverture gardée, originale ou redimensionnée |
| `GET, PATCH /api/volumes/{id}` | détail d'un tome (emplacements, problèmes, réglages effectifs, progression), modification de ses métadonnées (**admin**) |
| `GET, PUT, DELETE /api/volumes/{id}/progress` | page lue |
| `GET /api/volumes/{id}/pages` | pages dans l'ordre de lecture |
| `GET /api/pages/{id}/image` | une page : originale, ou `?w=<largeur>&f=avif\|jpeg` |
| `GET, POST /api/admin/scans`, `GET /api/admin/scans/{id}`, `.../logs` | historique, lancer un scan, une passe, son journal |
| `GET /api/admin/scans/current`, `POST .../current/cancel` | scan en cours, annulation |
| `GET /api/admin/metadata`, `POST /api/admin/metadata/identify` | état des sources, identification de toutes les séries |
| `GET /api/admin/storage` | chemins complets de la base et du cache |
| `GET, POST /api/admin/backups`, `GET, DELETE .../{nom}` | sauvegardes : liste (dossier, fréquence), en faire une, la télécharger, la supprimer |
| `POST /api/admin/backups/upload`, `POST .../{nom}/restore` | téléverser une sauvegarde (corps brut, en-tête SQLite exigé), restaurer (refusé pendant un scan, `invalid_backup`, `backup_too_new`) |

Tout le reste est l'application web : un fichier de `web/dist`, ou `index.html` pour une route de l'application (React Router). En debug, `web/dist` est lu sur le disque ; en release, il est intégré au binaire (`rust-embed`) : `npm run build` avant `cargo build --release`.

## Images

- **Largeurs fixes** (`render.rs`, `WIDTHS` : 160 à 2048 px) : le client demande la plus petite qui couvre sa largeur affichée en pixels réels ; une liste fermée borne le cache et empêche de faire calculer des milliers de variantes. Jamais d'agrandissement : une page plus étroite est servie telle quelle.
- **Formats** : AVIF pour les pages (`ravif`, qualité 70), JPEG pour les miniatures et les couvertures (qualité 85), redimensionnement Lanczos3, décodage plafonné en mémoire ; tout en Rust pur. Une page déjà en AVIF est servie telle quelle.
- **Cache disque** (`cache.rs`) indexé par (page, largeur, format, version du rendu) ; rien n'est jamais invalidé, le moins récemment lu part quand le plafond (`--cache-max-mib`, 2 Gio par défaut) est atteint. Au plus 4 rendus simultanés.
- **Webtoon** : les bandes sont servies en originale tant qu'elles ne dépassent pas trois fois la largeur utile (le navigateur les réduit sans coût serveur).

## Métadonnées

Fusion **champ par champ** (`shiori-db::metadata`) : champ édité (verrouillé) > fichiers (`index.json`, puis `ComicInfo.xml`, puis `Info` d'un PDF) > sources web dans l'ordre choisi. La vue fusionnée d'une série est recalculée et stockée à chaque changement, une fois par langue de l'interface (`merged`, `merged_en`), chaque compte lisant celle de sa langue ; celle d'un tome est calculée à la lecture (pour toute une série en deux requêtes).

Sources web (`shiori-meta`, désactivées par défaut) : AniList (GraphQL) et MangaDex (REST), appelées hors du runtime async, débit sous les limites publiées, `Retry-After` respecté, nom de client générique, seul le titre de la série envoyé. Identification automatique après chaque scan : un lien seulement si un seul candidat a exactement le même titre normalisé. Couvertures téléchargées depuis les seuls hôtes connus de chaque source, sans suivre de redirection (parade SSRF), 4 096 px de côté au plus.

## Serveur

- **axum** + **tokio**, compression brotli / gzip des réponses texte, journaux `tracing` (`-v`, `-vv`, ou `RUST_LOG`).
- **Job de scan** (`job.rs`) : une passe à la fois (bibliothèque, dossier ou fichier), fichiers scannés en parallèle (`--jobs`), progression en mémoire, annulation entre deux fichiers ; une passe interrompue par un arrêt brutal est close au lancement suivant.
- **Arrêt propre** sur Ctrl-C et SIGTERM (`docker stop`) : le scan en cours est annulé et sa passe close.
- **Authentification** (`server/auth.rs`) : cookie `shiori_session` `HttpOnly`, `SameSite=Lax`, `Secure` quand le proxy annonce `X-Forwarded-Proto: https` ; essais ratés comptés par nom (5 libres) et par adresse (20 libres), puis pause qui double jusqu'à 15 minutes (429 `rate_limited`, `Retry-After`) sans calculer le hash ; l'adresse est celle de la connexion, ou le dernier `X-Forwarded-For` quand la connexion vient d'un proxy local (loopback ou réseau privé). Jeton d'installation tiré au démarrage, affiché même avec `--quiet` tant qu'aucun compte n'existe. TOTP (`totp-rs`) : SHA-1, 6 chiffres, 30 s, un pas de tolérance. Écritures refusées si `Sec-Fetch-Site: cross-site` (CSRF).
- **Sauvegardes** : `--backup-dir` / `SHIORI_BACKUP_DIR` (défaut `backups` à côté de la base), `--backup-every-hours` (24, 0 : aucune), `--backup-keep` (7) ; `shiori backup` à la demande.
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
| PDF | hayro-interpret (`embed-fonts`) | Rust pur, analyse paresseuse (1 ms pour un PDF de 104 Mo une fois lu, contre 200 ms et tout en mémoire pour lopdf), décodeurs JPEG 2000, JBIG2, CCITT inclus ; les 14 polices standard (300 Kio) pour voir le texte en Helvetica non inclus |
| Hash | blake3 (`pure`) | rapide, sans code C |
| Images | image, ravif, imagesize | décodage et encodage AVIF / JPEG en Rust pur, dimensions sans décoder |
| HTTP sortant | ureq + rustls (`ring`) | client synchrone simple, TLS sans OpenSSL |
| Front embarqué | rust-embed | un seul binaire |
| Interface | React, Tailwind, shadcn/ui, TanStack Query | composants accessibles (Radix), état serveur simple |
