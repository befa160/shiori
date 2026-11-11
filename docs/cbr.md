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
- **libarchive non mesurée** : elle n'est pas installée sur la machine, et la compilation de ses sources a été refusée par la politique de sécurité de la session. Mesure possible si tu installes `libarchive-tools` (`bsdtar`).

## 5. Décisions (2026-11-11)

- **Q1a** : le CBR est géré dans la phase 2 (lot 2.8).
- **Q1b** : **cache d'extraction (B)**, appliqué aux seuls RAR solid ; les non solid sont lus directement (variante C, gratuite avec `rars`).
- **Q1c** : **`rars`**, le plus performant des candidats mesurables : pur Rust, donc aucune feature native ni dépendance système (principe 9) ; accès direct par index, plafonds et annulation intégrés. libarchive ne serait adoptée que si une mesure future la montrait nettement plus rapide, et alors derrière une feature `cbr-libarchive`.
- **Q1d** : `Toriko-v16.cbr` sert d'essai réel. Il manque un vrai CBR solid : à fournir si tu en as un.

## 6. Conception retenue (lot 2.8)

- `shiori-core::archive::rar::RarArchive` implémente le trait `Archive` existant (`entries`, `duplicates`, `open_entry`, `read_entry`) ; `archive_index` = index du membre dans l'archive, `entry_name_raw` = octets bruts du nom RAR (le `page_id` suit la même formule que pour le CBZ). Ordre naturel : les noms RAR4 utilisent `\` comme séparateur, à découper comme `/`.
- **Limites toujours posées** (désactivées par défaut dans `rars`) : `with_max_member_output_bytes(MAX_ENTRY_SIZE)` et `with_max_total_output_bytes(somme des tailles déclarées)`, plus un plafond de nombre d'en-têtes : une archive piégée échoue en erreur explicite au lieu de remplir la mémoire ou le disque.
- **Scan** : décodage en une seule passe en flux (`extract_to` avec un rappel par membre), jamais `read_members_at` sur tout le tome (il garderait tout en mémoire, 141 Mo pour Toriko) ; CRC vérifié par `rars`, en-têtes d'image lus comme pour le CBZ. `volumes.format = 'cbr'`. Nouveaux codes : `solid_archive` (info), `multivolume_archive` (error, `.part2.rar` non géré), `encrypted_entry` réutilisé.
- **Lecture d'une page, RAR non solid** : `read_member_at(archive_index)`, puis mêmes vérifications que le CBZ (nom brut, CRC).
- **Lecture d'une page, RAR solid** : cache `<cache>/extract/<volume_id>/<ordinal>` ; première demande d'un tome → une seule extraction en flux dans `<volume_id>.tmp/`, renommée à la fin (jamais de dossier partiel visible) ; les requêtes simultanées sur le même tome attendent cette extraction au lieu d'en lancer une autre ; taille de chaque fichier relue contre `pages.size` avant de servir. Éviction : les tomes extraits comptent dans le même plafond `--cache-max` que les images redimensionnées, le tome le moins récemment lu part en premier, **jamais le dernier ouvert** (sinon un tome solid plus gros que le plafond serait réextrait à chaque page). Un fichier encore ouvert (refus de suppression sous Windows) est sauté et retenté au passage suivant.
  - shortcut : une requête attend la fin de l'extraction complète (environ 2 s pour 140 Mo), même pour la page 1 ; servir les pages au fil de l'extraction viendra si la première ouverture paraît lente.
- Parcours des dossiers (CLI et job) : `.cbr` ajouté à `.cbz`, insensible à la casse.
- Tests : fixtures RAR solid et non solid générées par `rars` (feature `write` en dev-dependency de `shiori-testkit` seulement), test de l'invariant d'ordre de lecture en HTTP sur les deux, archive solid tronquée → pages manquantes en erreur explicite.
