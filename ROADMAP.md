# Roadmap Shiori

Document vivant : il est mis à jour à chaque PR qui fait avancer une phase (cases cochées, décisions, questions résolues). Le détail des phases 1 à 11 (plans, constats, lots, bilans) est dans [ROADMAP-HISTORIQUE.md](ROADMAP-HISTORIQUE.md) ; ici restent l'état, la suite et les décisions.
Légende : ✅ fait · 🚧 en cours · ⏳ à faire · ❓ décision attendue.

| Phase | Contenu | État |
|---|---|---|
| 0 | Outillage (Rust, structure, Ponytail, Graphify, UI/UX Pro Max, MCP shadcn + Playwright) | ✅ |
| 1 | CLI de scan d'intégrité + schéma SQLite + CI linux/windows, puis graphe Graphify | ✅ |
| 2 | API axum + streaming/redimensionnement des images + parseur de noms + job de scan + API d'administration des scans + CBR | ✅ |
| 3 | Lecteur web (sens de lecture, webtoon, doubles pages) + front embarqué + progression de lecture | ✅ |
| 4 | Bibliothèque façon Plex + statuts/filtres + menu d'administration (scans, journaux) + DESIGN_SYSTEM.md | ✅ |
| 5 | Métadonnées web + édition manuelle | ✅ |
| 6 | Distribution : image Docker, PWA finalisée, binaires Linux/Windows en release, Dependabot, documentation (technique, installation, fonctionnelle), thèmes de l'interface (sombre, clair, OLED, sépia) | ✅ |
| 7 | Authentification et comptes : comptes locaux, rôles, progression et réglages par compte, double authentification, jetons d'API, sauvegarde et restauration | ✅ |
| 8 | Lecture des PDF sans marges | ✅ |
| 9 | Maintenance de la bibliothèque (surveillance des dossiers, scan complet périodique, santé) + documentation VitePress bilingue + bilan V1 | ✅ |
| 10 | Retours de la vraie bibliothèque (NAS) : scan tolérant, sources de métadonnées et Wikipédia FR, série lue/non lue, lecture rapide | ✅ |
| 11 | Avant la V1 : hors ligne qui prévient quand il ne marchera pas, « Tome suivant » vers un tome en erreur, promesse de compatibilité et attestations, `v1.0.0-rc.1` | ✅ |
| V1 | Vérifications sur vrai matériel, audit, captures de la documentation, attestations, promesse de compatibilité (voir « Maturité pour une V1 » dans l’historique) ; `v1.0.0` | ✅ |

## Méthode de travail

- **Une branche par lot, une PR par lot**, jamais de commit direct sur `main` (sauf exception validée). Noms de branche : `feat/…`, `fix/…`, `chore/…`, `ci/…`, `docs/…`, `test/…`.
- Chaque PR : description (contexte, changements, tests, captures si UI), CI verte, `cargo fmt` + `cargo clippy -D warnings` propres, **couverture ≥ 80 % lignes et branches** (job `coverage`), ROADMAP.md mis à jour si la PR fait avancer la phase.
- Fusion par toi après relecture ; squash merge par défaut (historique de `main` = une ligne par lot).
- **Graphify** : graphe construit à la fin de la phase 1 ; les hooks git le maintiennent à jour, plus de `GRAPHIFY_SKIP_HOOK`.
- Avant chaque phase : plan détaillé ici (section « Prochaine phase »), validation, puis code ; une phase terminée part dans l'historique avec son bilan.


---

## Prochaine phase : 1.1 (⏳ plan validé, à coder)

Shiori `v1.0.0` couvre l'usage visé (bibliothèque chez soi, lecture en streaming et hors ligne sur la Tab A9 et le A55, comptes de la famille). La 1.1 réunit les statistiques de lecture (onglet prévu par CLAUDE.md) et une **application Android** (enveloppe Tauri 2) qui bascule seule entre l'adresse locale et l'adresse publique. OPDS repasse au backlog (2026-11-23).

Constats avant le plan :
- `reading_progress` ne garde que le dernier état par tome (`updated_at`, `finished_at`) : les tomes terminés par semaine s'en déduisent, **pas les pages lues ni le temps**. Il faut un journal d'activité, sans historique avant la 1.1.
- La barre du bas a déjà ses 4 onglets (Accueil, Séries, Recherche, Compte).
- Tauri 2 sert sur Android un protocole maison à `http://<schéma>.localhost` par un gestionnaire Rust asynchrone ; le greffon de mise à jour de Tauri ne fonctionne pas sur mobile.
- Le front appelle `/api` en relatif et la session tient dans un cookie (`SameSite=Lax`) ; la connexion renvoie aussi le jeton de session, et l'API l'accepte en `Bearer`. Une application qui chargerait le front depuis sa propre origine serait « cross-site » : cookie non envoyé, images (`<img>`) sans en-tête d'authentification.
- Le hors-ligne repose sur le service worker (`public/sw.js`) et le cache `shiori-offline-v1` ; un service worker sur une origine de protocole maison de la WebView Android n'est pas garanti.
- Tauri sous Linux tire webkit2gtk (bibliothèque C) : l'application ne peut pas entrer dans le workspace `backend/` sans casser `cargo test --workspace` et la règle du pur Rust. Un build Android (NDK, Gradle) dépasse aussi la mémoire de la machine de dev (WSL 3,8 Gio) : l'APK se construit en CI.

Proposition d'architecture (Q62) : l'application est un **relais**. La WebView charge `http://shiori.localhost/` ; un gestionnaire Rust transmet chaque requête (front compris) au serveur joignable, en ajoutant `Authorization: Bearer` avec le jeton de session qu'il a relevé à la connexion. Le front est donc toujours celui du serveur (jamais de décalage de version), tout reste « same-origin » pour la WebView, et **le serveur ne change pas**. L'application n'ajoute qu'un écran de réglage des deux adresses.

Lots :
- ✅ **1.1a Statistiques, serveur** (#133) : table `reading_activity(user_id, day, pages, seconds, finished)` remplie à chaque progression enregistrée (pages = avance depuis l'état précédent, temps = écart entre deux enregistrements s'il est court) ; `GET /api/me/stats` (par jour, regroupé en semaines par l'interface : tomes terminés, pages, temps ; séries terminées ; totaux) ; tomes terminés avant la 1.1 repris depuis `finished_at` ; fuseau de l'appareil reçu avec la progression (Q59) ; « Marquer comme lu » n'entre pas dans les statistiques (`stats: false`).
- 🚧 **1.1b Statistiques, interface** (`feat/stats-page`) : page `/stats`, entrée « Statistiques » dans la sidebar et lien depuis la page Compte (Q60), barres en CSS (pas de bibliothèque de graphiques), i18n, e2e, captures 4 tailles, documentation FR/EN.
- **1.1c Application, essai** (go / no-go) : dossier `app/` (projet Tauri 2 à part, son propre workspace cargo), relais vers une seule adresse, connexion et lecture d'un webtoon sur la Tab A9 ; mesure de fluidité à travers le relais ; job CI qui construit l'APK de debug. Arrêt et retour au split DNS si l'essai n'est pas concluant.
- **1.1d Application, adresses et session** : écran de réglage (adresse locale, adresse publique), bascule automatique sur Wi-Fi nommé (Q66, greffon Kotlin), jeton de session gardé dans le stockage privé de l'application (Q67), déconnexion qui l'efface ; logique du relais dans un module Rust testé sans Tauri (couverture ≥ 80 %).
- **1.1e Application, hors ligne** : le relais garde sur disque le front et ce que « Télécharger » récupère, et le sert quand aucune adresse ne répond ; la progression lue hors ligne repart à la reconnexion, datée comme dans la PWA (`read_at`) (Q62, Q63).
- **1.1f Application, distribution** : APK signé dans la release GitHub, attestation de provenance, mises à jour par Obtainium (Q64), documentation FR/EN (installation, réglage, mise à jour).

Décisions (2026-11-23) :
- **Q58** Temps compté seulement si moins de 5 minutes séparent deux pages ; une avance de plus de 50 pages d'un coup (saut au curseur) compte 0 page.
- **Q59** Jours et semaines dans le fuseau de l'appareil, envoyé avec la progression.
- **Q60** Statistiques sur mobile : lien depuis la page Compte, barre du bas inchangée ; entrée dans la sidebar à partir de `lg`.
- **Q61** Statistiques strictement personnelles.
- **Q62** Application en relais, avec cache disque : il garde le front et les tomes téléchargés, donc l'application s'ouvre et lit hors ligne ; besoin exprimé : hors ligne et bascule automatique par détection du réseau.
- **Q63** Hors ligne de l'application par le cache disque du relais (pas de service worker dans la WebView).
- **Q64** APK signé dans la release GitHub, mises à jour par Obtainium ; clé de signature créée par l'utilisateur, dans les secrets GitHub avec une copie hors ligne.
- **Q65** Android seulement (Tab A9, A55).
- **Q66** Adresse locale seulement sur un Wi-Fi nommé (liste réglée dans l'application) **et** si elle répond (`/api/health`, 1,5 s), sinon adresse publique ; réévalué à chaque changement de réseau (greffon Kotlin : nom du Wi-Fi et changements de réseau, permission de localisation demandée avec une explication). Raison : sur un autre Wi-Fi à la même plage d'adresses, un autre appareil recevrait la session en clair.
- **Q67** Jeton de session dans le stockage privé de l'application, sans Keystore.

## Planning après la V1

Le backlog ci-dessous, rangé par version (S : une PR, M : 2 ou 3 PR, L : une phase). Chaque version garde son plan détaillé validé avant de coder ; l'ordre peut bouger.

| Version | Contenu | Taille |
|---|---|---|
| 1.1 | Statistiques de lecture ; application Android (Tauri 2, relais avec bascule locale / publique) | M + L |
| 1.2 | Lecteur : luminosité, rognage des marges blanches (calculé côté serveur, principe 3), marque-pages ; édition d'un tome dans l'interface | S + M + M + S |
| 1.3 | Nouveautés suivies ; listes « À lire » par compte, puis collections partagées et relations entre séries | M + S + M |
| 1.4 | Liens d'invitation ; sources MangaUpdates et Comic Vine ; actualisation périodique des fiches | M + M + S |
| 1.5 | En-tête `Remote-User` (pour les autres installations, inutile ici) | S |
| plus tard | OPDS (lecture et progression, sur les jetons d'API) | M |

Sur déclencheur, sans version : pagination de la liste des séries (quelques milliers de séries), option JPEG des pages (pauses sur le NAS), signature Windows et macOS (décision de budget), icône dans la zone de notification.

Entretien courant, sans phase : la routine cloud du mardi (10 h, heure d'hiver) regroupe les PR Dependabot dans une PR vérifiée ; une release corrective (`1.0.x`) suit quand elle apporte quelque chose ; le reste (Triage X, e2e instable, Safari iOS, registre shadcn) se fait au fil des lots.

## Backlog : idées et demandes (sans ordre, sauf mention)

Regroupe tout ce qui a été noté « plus tard » pendant les phases 1 à 11 (le détail d'origine reste dans l'historique).

### Lecture et bibliothèque

- **Statistiques de lecture** : tomes et pages lus par semaine, séries terminées, temps de lecture estimé ; la progression est déjà horodatée par compte. Candidat 1.1.
- **Listes** : « À lire » par compte, collections partagées choisies par l'administrateur, relations entre séries (suite, édition, spin-off).
- **Nouveautés suivies** : marquer une série comme suivie et voir à l'accueil les tomes ajoutés depuis la dernière visite.
- **Marque-pages** dans un tome (plusieurs par tome, avec une note), en plus de la progression.
- **Réglages d'image du lecteur** : luminosité (lecture le soir sur l'écran LCD de la Tab A9), rognage des marges blanches d'un scan.
- **Édition d'un tome** dans l'interface (titre, numéro, couverture) : l'API existe.
- **Pagination de la liste des séries**, quand une bibliothèque dépasse quelques milliers de séries (voir les limites connues).
- Non retenu : EPUB (Shiori lit des images ; le PDF l'est depuis la phase 8).

### Accès et comptes

- **OPDS**, sur les jetons d'API (lecture et progression seulement) : lire depuis Mihon, Panels, KOReader ou une liseuse. Reporté le 2026-11-23 au profit de l'application Android. Points relevés : Basic à accepter pour un jeton (KOReader, Panels), seulement sous `/api/opds` et sans cookie (CSRF), sans `WWW-Authenticate` ailleurs (fenêtre du navigateur) ; PSE met un numéro de page dans l'URL (exception au principe 1, redirection `no-store`) ; téléchargement du fichier d'origine en flux.
- **Liens d'invitation** (conception de la phase 7) : l'administrateur génère `/invite/<jeton>` (256 bits, seul le hash en base, usage unique, 7 jours par défaut) avec le rôle et les bibliothèques du futur compte ; la personne choisit son nom et son mot de passe ; liste et révocation dans Administration → Comptes ; aucune inscription libre.
- **En-tête `Remote-User`** d'un proxy d'authentification (Authelia, Authentik ; Q42, Q54) : option `--auth-header`, refusée sans `--trusted-proxy` (l'en-tête n'est cru que d'un proxy déclaré) ; session ouverte pour le compte de ce nom ; compte inconnu refusé (page de connexion), jamais créé d'office. Pas utile chez l'utilisateur (Nginx Proxy Manager sans authentification devant).

### Métadonnées

- **Sources** : MangaUpdates, Comic Vine.
- **Actualisation périodique** des fiches liées (aujourd'hui : à la demande, bouton Actualiser).

### Distribution

- **Signature Windows** (Authenticode : SignPath Foundation, Azure Trusted Signing ou certificat OV/EV) et **macOS** (Developer ID et notarisation, compte Apple payant) : supprimeraient l'avertissement SmartScreen et l'étape `xattr`. Questions ouvertes : budget annuel, dossier SignPath ou abonnement Azure, compte Apple à quel nom. Les attestations de provenance (gratuites) sont faites depuis la 1.0.
- **Icône dans la zone de notification** (rouvrir, quitter) et fenêtre console masquée sous Windows, si le double-clic ne suffit plus.

### Qualité et entretien

- Format des pages sur une petite machine : AVIF gardé (Q55) ; option JPEG (`--page-format`) seulement si les pauses reviennent en feuilletant sur le NAS.
- Les deux tomes 7z de Triage X sont à reconvertir par l'utilisateur (signalés `unsupported_archive`).
- À surveiller : test e2e « tome suivant » instable vu une fois en CI (phase 4) ; `:root:has()` sur un vrai Safari iOS.
- Front : registre shadcn et paquet `cn` (noté en phase 4).

## Limites connues

Décisions assumées, chacune marquée `shortcut:` dans le code avec son seuil (18 au 2026-11-22 ; liste à jour : `/ponytail-debt`). Aucune n'est atteinte par l'usage visé.

- **Archives** : CRC et tailles 32 bits pour le contrôle zip64 (> 4 Gio) ; deux lectures du fichier au scan (hash puis entrées) ; PDF lu en entier en mémoire ; couleur de peinture d'un masque PDF ignorée ; PDFDocEncoding lu comme du Latin-1 ; CBR multivolume refusé ; pages AVIF d'origine jamais redimensionnées (pas de décodeur AVIF pur Rust).
- **Noms et textes** : chiffres pleine chasse non reconnus dans les noms ; `NOCASE` ne replie que l'ASCII ; chemins non UTF-8 convertis avec perte ; noms de compte sans normalisation Unicode.
- **Serveur** : 4 rendus d'image en parallèle ; couvertures rendues sans cache disque ; mémoire des vignettes bornée à 1 000 adresses ; port pris supposé tenu par un Shiori déjà lancé ; arrêt qui attend la fin d'une identification en cours ; défaut global de `page_turn` ignoré pour un tome hors bibliothèque.
- **Interface** : liste des séries sans pagination ; résumé replié d'après sa longueur ; caches du service worker bornés en nombre d'entrées ; décalage des doubles pages non retenu.

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
- 2026-11-16 : binaire lancé sans argument (double-clic sous Windows) = `shiori serve`, une erreur reste affichée jusqu'à Entrée ; bascule adresse locale / publique par DNS local (split DNS), la bascule dans l'application attendra une application native.
- 2026-11-16 : PWA recentrée sur le streaming (Q38 révisée) ; hors ligne sur demande, tome par tome, page par page (lot 6.5b) ; 2 pages préchargées, d'après une mesure sur 4G lente.
- 2026-11-16 : tomes téléchargés pour le hors-ligne (lot 6.5b) ; authentification et comptes ajoutés en phase 7 (à planifier).
- 2026-11-16 : signature des binaires (Windows, macOS) et attestations des artefacts (Linux, image Docker) ajoutées à la roadmap, à planifier.
- 2026-11-16 : phase 6 terminée (distribution, thèmes, PWA et hors-ligne, documentation) ; version `v0.1.0`.
- 2026-11-16 : version `v0.1.1` : correctifs de l'audit (dont le scan d'une racine illisible qui vidait la bibliothèque), recherche et filtre par tag, double-clic qui ouvre l'interface, Administration → Données ; plan de la phase 7 proposé.
- 2026-11-17 : plan de la phase 7 validé : comptes créés par l'administrateur (liens d'inscription notés pour plus tard), page d'installation avec jeton, sessions de 90 jours glissants, réglages de lecture des séries par compte, nouvelles bibliothèques fermées aux lecteurs existants ; double authentification (TOTP) et jetons d'API ajoutés à la phase (lots 7.5 et 7.6), `Remote-User` plus tard. Une future application mobile passera par la connexion (session en `Bearer`, tous les droits du compte), pas par un jeton d'API, qui reste limité à la lecture et à la progression.
- 2026-11-17 : lecture des PDF sans marges ajoutée en phase 8 (images incluses servies comme des pages, pas de conversion en CBZ, page sans image ignorée avec un avertissement, pages en plusieurs morceaux recomposées, métadonnées du PDF lues) ; la maintenance devient la phase 9.
- 2026-11-18 : phase 7 terminée (comptes et rôles, progression par compte, double authentification, jetons d'API, sauvegarde quotidienne et restauration dans l'interface) ; version `v0.2.0`.
- 2026-11-18 : phase 8 terminée (lecture des PDF sans marges : images incluses servies comme des pages, pages en plusieurs images recomposées, diagnostics et métadonnées du PDF) ; version `v0.3.0`.
- 2026-11-19 : phase 9 terminée (surveillance des dossiers, scan complet périodique, santé des bibliothèques, documentation VitePress bilingue) ; bilan de maturité pour une V1 proposé.
- 2026-11-19 : correctifs de l'audit avant la V1 (jetons d'API avec mot de passe, progression hors ligne datée, restauration sans scan concurrent, limites de connexion, `--trusted-proxy`) ; version `v0.4.0`.
- 2026-11-20 : phase 10 planifiée d'après le premier essai sur la vraie bibliothèque (NAS) : scan tolérant, sources de métadonnées et Wikipédia FR, série lue/non lue, lecture rapide ; 7z signalé seulement, pastille « Lu » gardée.
- 2026-11-20 : phase 10 terminée (scan tolérant, Wikipédia et métadonnées, série lue, rendu anticipé, zoom au pincement et à la souris) ; version `v0.5.0`.
- 2026-11-21 : titre affiché tiré des fichiers puis des noms de fichiers et de dossiers, plus jamais d'un titre web (remplace Q34) ; retours d'usage de la v0.5.0.
- 2026-11-21 : version `v0.5.1` : titres des séries et « Continuer la lecture » d'un appareil à l'autre (sans rescan).
- 2026-11-22 : plan de la phase 11 validé (avant la V1 : garde du hors ligne, « Tome suivant » marqué en erreur, promesse de compatibilité ; `Remote-User` après la V1, AVIF gardé) ; idées pour après la V1 notées (OPDS, statistiques, listes, nouveautés suivies, marque-pages, réglages d'image).
- 2026-11-22 : phase 11 terminée (garde du hors ligne, « Tome suivant » signalé, promesse de compatibilité, attestations) ; version `1.0.0` dans le code, tag `v1.0.0-rc.1` à poser après l'audit.
- 2026-11-22 : audit avant la V1 (#122) ; préversion `v1.0.0-rc.1` publiée ; captures d'écran de la documentation.
- 2026-11-22 : essai de la rc.1 validé sur les appareils ; version `v1.0.0`.
- 2026-11-23 : audit de sécurité et de qualité après la 1.0 : CSRF `same-site` et retrait du TOTP corrigés (#127), `server.rs` découpé (#128), gestes du lecteur et toasts d'erreur mis en commun (#129) ; `shiori health` et `HEALTHCHECK` dans l'image Docker (Docker ne redémarre pas seul un conteneur `unhealthy`).
- 2026-11-23 : version `v1.0.1` (correctifs de l'audit, health check Docker).
- 2026-11-23 : backlog rangé par version (1.1 statistiques et application Android, OPDS reporté, 1.2 lecteur, 1.3 listes et nouveautés, 1.4 invitations et métadonnées, 1.5 `Remote-User`) ; le reste attend un déclencheur.
- 2026-11-23 : plan de la 1.1 validé (Q58 à Q67) : statistiques personnelles dans le fuseau de l'appareil ; application Android en relais avec cache disque, Wi-Fi nommé pour l'adresse locale, APK par GitHub et Obtainium.
