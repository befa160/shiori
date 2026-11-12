# Note CBR (Q1)

Rédigée le 2026-11-11 avant la phase 2. Objet : choisir comment Shiori lira les CBR (archives RAR) sans trahir les principes 1, 4, 5, 6 et 9. Questions tranchées le 2026-11-11 (§5), conception retenue au §6.

## 1. Le problème : RAR solid n'a pas d'accès aléatoire

- **RAR non solid** : chaque fichier est compressé seul. On peut sauter d'en-tête en en-tête (RAR n'a pas d'annuaire central comme le zip, il faut parcourir les en-têtes locaux) puis décompresser uniquement l'entrée voulue. L'accès direct est possible, au prix d'un index des positions à stocker au scan.
- **RAR solid** : tous les fichiers forment un seul flux compressé qui partage le même dictionnaire. Pour décompresser la page *n*, il faut décompresser les pages 0 à *n*−1. Ouvrir la page 200 d'un tome de 100 Mo, c'est décompresser presque tout le tome, **à chaque requête**. Le principe 6 (lecture directe d'une entrée) est donc impossible à tenir tel quel.
- Les CBR qu'on trouve en pratique sont souvent solid (option par défaut de nombreux outils de création).

Conséquence : servir un CBR solid page par page exige une copie décompressée quelque part. La vraie question est **quand** on la fait et **combien de temps** on la garde.

## 2. Les deux stratégies

Dans les deux cas, rien n'est écrit à côté des fichiers sources (principe 5) : tout va dans le dossier de données de Shiori.

### A. Conversion CBR→CBZ au scan

Au scan d'un `.cbr`, on décompresse toutes les entrées (de toute façon nécessaire pour vérifier les CRC et lire les en-têtes d'image, comme pour un CBZ) et on les écrit au passage dans un zip **sans recompression** (méthode `stored`, les images sont déjà compressées) : `<données>/converted/<volume_id>.cbz`.

- Ensuite, tout le reste du code est celui du CBZ : `archive_index`, CRC, lecture directe, redimensionnement. Un seul chemin de lecture à tester.
- Identités inchangées (principes 1 et 8) : `volume_id` = hash du `.cbr` d'origine, `page_id` = hash(`volume_id`, nom brut de l'entrée RAR). Le zip converti n'est qu'un cache : supprimé, il est reconstruit à l'identique et les IDs ne bougent pas.
- Coût : **l'espace disque des CBR est doublé**, de façon permanente. Le scan écrit ce volume une fois (le temps de décompression, lui, est déjà payé par la vérification d'intégrité).
- Première lecture instantanée, aucune latence imprévisible.

### B. Cache d'extraction à la demande

Le scan vérifie le CBR (décompression complète, puis on jette le résultat) et ne garde que les métadonnées. À la première ouverture d'un tome, on l'extrait dans `<cache>/extract/<volume_id>/`, cache borné en taille, avec éviction des tomes les moins récemment lus.

- Disque borné (taille du cache configurable).
- Première ouverture lente : la page 0 peut sortir vite (début du flux), mais sauter directement à la page 200 attend la décompression de tout ce qui précède. Il faut gérer les requêtes concurrentes pendant l'extraction, l'éviction pendant une lecture, les extractions interrompues : plus de code et plus d'états à tester.
- Chaque tome est décompressé deux fois (scan, puis première lecture), puis de nouveau après chaque éviction.

### Variante C (optimisation, plus tard)

Lire les RAR **non solid** directement (index des positions stocké au scan) et ne convertir que les solid. Économise le disque pour les non-solid, mais ajoute un second chemin de lecture. À ne faire que si la mesure montre que l'espace disque pose problème.

### Option D : ne pas gérer le CBR

L'utilisateur convertit lui-même ses CBR en CBZ avec un outil externe. Zéro code, zéro risque de licence. Le scan signale juste les `.cbr` rencontrés (code `unsupported_format`, avertissement).

## 3. Bibliothèques de décompression RAR

| Option | Licence | Langage | Formats | Principe 9 | Verdict |
|---|---|---|---|---|---|
| `unrar` (crate 0.5.8, embarque le source C++ de RARLAB via `unrar_sys`) | Enveloppe MIT/Apache, mais le code embarqué est sous **licence UnRAR** : gratuite, mais interdit d'utiliser ce code pour créer un archiveur compatible RAR ou recréer l'algorithme de compression. Non approuvée OSI, classée non libre par Debian, incompatible GPL | C++ | Référence : tout RAR | Non : C++ compilé, donc seulement derrière une feature | Écarté : licence non libre dans un projet MIT, plus C++ |
| libarchive via `compress-tools` (0.16.1) | **BSD-2-Clause** (bibliothèque), MIT/Apache (enveloppe) | C | RAR4 et RAR5 en lecture, solid compris, sans chiffrement RAR5 | Non : il faut libarchive installée (pkg-config sous Linux, vcpkg sous Windows MSVC), ce qui complique la CI Windows et la distribution | Repli acceptable derrière une feature `cbr-libarchive` |
| `rars` (0.12.4, bitplane) | **Apache-2.0** | **Rust pur** (dépendances : `crc32fast`, `aho-corasick`, crypto optionnelle) | Annoncé : toutes les versions de RAR 1.x à 7, solid, multivolume, lecture et écriture | Oui, sans feature native | Candidat principal, avec réserves ci-dessous |
| `unrar-rs` (0.16.1, rarpar) | **GPL-3.0 + restriction UnRAR** | Rust | RAR4/RAR5 | Oui | Écarté : GPL (et dérivé du source UnRAR) |
| `rar` (0.4.0, Roba1993) | MIT | Rust | Partiel, peu maintenu | Oui | Écarté : incomplet |
| `rar-codec` (0.1.10) | Apache-2.0 | Rust (no_std) | RAR4/RAR5, solid | Oui | Trop jeune (créé le 2026-10-23, < 1 000 téléchargements) |

Réserves sur `rars`, vérifiées le 2026-11-11 dans son dépôt :

- Projet récent (créé en mai 2026, « expérience de développement agentique » selon son auteur), environ 97 000 téléchargements sur 90 jours ; son README dit lui-même qu'il manque de tests à grande échelle. Il a un dossier `fuzz/` et des fixtures d'archives piégées.
- **Provenance** : la spécification `bitplane/rar-research` se présente comme « clean-room » et écarte explicitement le source UnRAR et les décodeurs de 7-Zip (« développés à partir du source unRAR, ils portent la même restriction »). Une partie a été établie en désassemblant des binaires WinRAR et UNRAR.EXE (fonctions `FUN_…`), surtout pour l'écriture et des détails anciens. C'est de l'ingénierie inverse pour l'interopérabilité, pas une copie du code UnRAR : la restriction UnRAR ne s'applique pas. Shiori n'utilise que la lecture (`default-features = false` : ni `write`, ni `recovery`, ni `encryption`, ni `parallel`).
- Aucun `unsafe` (son workspace l'interdit, comme le nôtre). Plafonds de sortie par entrée et par appel (`with_max_member_output_bytes`, `with_max_total_output_bytes`), annulation (`ReadCancellation`) : les garde-fous contre les archives piégées existent déjà. On garde `catch_unwind` autour des appels, comme pour les décodeurs d'image.

## 4. Mesures sur un vrai CBR (2026-11-11)

Fichier fourni : `Toriko-v16.cbr`, 142 Mo, RAR 1.5-4.x, **non solid**, 210 pages JPEG de 1200 px dans un sous-dossier, chemins séparés par `\`. Build release, `rars` 0.12.4, 8 cœurs, fichier en cache.

| Mesure | Toriko (non solid) | Copie solid RAR4 | Copie solid RAR5 |
|---|---|---|---|
| Ouverture (lecture des en-têtes) | 2,2 ms | 1,4 ms | 1,3 ms |
| Page 1 seule | 17 ms | 14 ms | 13 ms |
| Page du milieu seule | 15 ms | **1,08 s** | **1,16 s** |
| Dernière page seule | 6 ms | **2,0 s** | **2,1 s** |
| Tout le tome page par page | 2,9 s | **218 s** | (arrêté) |
| Tout le tome en une passe | 2,5 s | 2,1 s | (non mesuré) |
| Vérification complète (`rars test`, CLI) | 2,4 s sur 1 cœur, 0,66 s sur 8 (11 à 34 Mo de mémoire) | | |

Les copies solid ont été recréées avec `rars a --solid` à partir des pages extraites de Toriko (je n'ai pas de vrai CBR solid).

Conclusions :

- **RAR non solid** : `read_member_at(index)` ne décode que l'entrée demandée. L'accès direct marche comme en CBZ (6 à 17 ms par page), sans copie ni cache. Le principe 6 est respecté.
- **RAR solid** : chaque page coûte la décompression de tout ce qui la précède. Servir page par page est inutilisable (218 s pour un tome) ; une passe unique coûte 2 s. D'où le cache d'extraction.

**Comparaison avec libarchive** (`bsdtar` 3.7.4 de la distribution, même fichier, même machine) :

| Mesure sur Toriko | libarchive (`bsdtar`) | `rars` |
|---|---|---|
| Liste des entrées | 10 à 60 ms (lancement compris) | 10 à 20 ms (CLI) |
| Extraction complète | 1,9 à 2,7 s | 2,6 s sur 1 cœur, 0,7 à 1,6 s sur 8 |
| Page 1 / 105 / 210 seule | 19 / 19 / 12 ms (dont environ 2 ms de lancement du processus) | 15 / 12 / 5 ms (bibliothèque, archive déjà ouverte) |
| Résultat | | octets identiques à ceux de `bsdtar` sur les 210 pages |
| Copies solid RAR4 et RAR5 créées par `rars` | **illisibles** : « RAR solid archive support unavailable », « Declared solid file, but no window buffer initialized yet » | lues sans erreur |

- Non solid : performances équivalentes sur un cœur, `rars` plus rapide dès qu'il peut paralléliser, et sans C.
- Solid : impossible de comparer. On ne sait pas si l'échec vient de libarchive (son lecteur RAR4 solid est connu pour être limité) ou si `rars` écrit des RAR solid hors norme ; il faudrait WinRAR ou unrar pour trancher. **Conséquence** : les chiffres solid ci-dessus et les fixtures solid générées par `rars` ne prouvent pas la lecture de vrais RAR solid. Un vrai CBR solid (créé par WinRAR) est nécessaire avant de clore le lot 2.8.

## 5. Décisions (2026-11-11)

- **Q1a** : le CBR est géré dans la phase 2 (lot 2.8).
- **Q1b** : **cache d'extraction (B)**, appliqué aux seuls RAR solid ; les non solid sont lus directement (variante C, gratuite avec `rars`).
- **Q1c** : **`rars`**, aussi rapide que libarchive sur un cœur et plus rapide en parallèle (mesures §4), en Rust pur, donc sans feature native ni dépendance système (principe 9) ; accès direct par index, plafonds et annulation intégrés. libarchive est écartée.
- **Q1d** : `Toriko-v16.cbr` sert d'essai réel. Il manque un vrai CBR solid créé par WinRAR : **condition pour clore le lot 2.8**.

## 6. Conception réalisée (lot 2.8)

- `shiori-core::archive::rar::RarArchive` implémente le trait `Archive` ; `archive::open` choisit CBZ ou CBR d'après les premiers octets (`Rar!\x1a\x07`), jamais l'extension : un zip renommé `.cbr` reste un zip (`volumes.format` suit le contenu). `entries()` liste **tous** les membres dans l'ordre de l'archive (`entries[i].index == i`, l'index de `read_member_at`) ; `entry_name_raw` = octets bruts du nom RAR, le `page_id` suit la même formule que pour le CBZ. Les noms RAR 1.5-4.x séparent les dossiers par `\`, remplacé par `/` dans le nom affiché.
- Doublons de nom : le RAR les garde tous ; le scan ne fait une page que de la **dernière** occurrence (comme `rars::read_member`), `duplicate_entry_*` signalé comme pour le zip.
- **Limites** : plafond de 1 000 000 en-têtes à l'ouverture, `with_max_total_output_bytes(somme des tailles déclarées)` à chaque décodage, et chaque page écrite dans un tampon borné à sa taille déclarée (au-delà, ou en deçà : `size_mismatch`). Une entrée annoncée au-delà de `MAX_ENTRY_SIZE` n'est jamais mise en mémoire (`entry_too_large`) ; dans une archive solid elle est décodée vers `io::sink()` pour garder l'historique des suivantes. Pas de `with_max_member_output_bytes` : `rars` en ferait une erreur fatale qui casserait toutes les pages suivantes d'un solid.
- **Scan** : nouvelle méthode du trait `read_each(indices, visit)`, une seule passe `extract_with_control_and_errors`, un membre en mémoire à la fois, remis une fois son CRC vérifié (quand `rars` passe au membre suivant). Non solid : un membre en erreur (CRC, chiffré) n'abîme que sa page, la passe continue. Solid : une erreur est fatale ; la page fautive porte l'erreur, chacune des suivantes « décodage interrompu plus tôt dans l'archive ». Le CBZ passe par l'implémentation par défaut (une lecture par entrée), l'examen d'une entrée est coupé en deux (`classify` sans lire, `check` sur le contenu).
- Codes : `solid_archive` (info), `multivolume_archive` (erreur, aucune page : un membre coupé entre `.part1` et `.part2` n'est pas lisible seul), `encrypted_entry` réutilisé. Un RAR coupé est refusé dès l'ouverture (`rars` lit tous les en-têtes) : `archive_truncated` pour le fichier.
- **Lecture d'une page, RAR non solid** : `read_member_at(archive_index)`, mêmes vérifications que le CBZ (nom brut, CRC, taille).
- **Lecture d'une page, RAR solid** : `Images::extracted`, cache `<cache>/extract/<volume_id>/<archive_index>`. Première demande d'un tome : une passe `read_each` qui écrit toutes ses entrées dans `<volume_id>.tmp/`, renommé à la fin (jamais de dossier partiel visible, un échec ne laisse rien) ; une page illisible manque simplement et sa lecture est une erreur explicite. Chaque fichier est relu contre `pages.size` avant d'être servi. Une seule extraction à la fois (verrou global) : les requêtes simultanées l'attendent puis lisent le disque. Éviction : les tomes extraits et les rendus partagent le plafond `--cache-max-mib`, le moins récemment lu part en premier (date de lecture = la plus récente de ses pages), **jamais le dernier tome lu**. Un tome évincé est d'abord renommé `<id>.del` puis effacé : si le renommage échoue (page ouverte sous Windows), il reste entier et compté, retenté à l'éviction suivante. Les `.tmp` et `.del` restés d'un arrêt brutal sont effacés à l'ouverture du cache.
  - shortcut : une requête attend la fin de l'extraction complète, même pour la page 1 (mesures au §7) ; servir les pages au fil de l'extraction viendra si la première ouverture paraît lente. Une extraction à la fois : un second tome solid attend le premier.
- Parcours des dossiers (CLI et job) : `.cbr` ajouté à `.cbz`, insensible à la casse.
- Tests : fixtures RAR 4 et 5, solid et non solid, générées par `rars` (features `write` et `encryption` dans `shiori-testkit` seulement, jamais dans le binaire) ; invariant d'ordre de lecture en HTTP sur les quatre ; CRC faux, chiffrement, doublons, multivolume, solid abîmé au milieu (pages suivantes `broken`), RAR coupé, cache d'extraction (éviction, échec, restes).

