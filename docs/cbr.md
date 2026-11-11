# Note CBR (Q1)

Rédigée le 2026-11-11 avant la phase 2. Objet : choisir comment Shiori lira les CBR (archives RAR) sans trahir les principes 1, 4, 5, 6 et 9. Rien n'est codé tant que les questions de la fin ne sont pas tranchées.

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

Réserves sur `rars` :

- Projet récent (créé en mai 2026, « expérience de développement agentique » selon son auteur), environ 97 000 téléchargements sur 90 jours, son README dit lui-même qu'il « manque de tests à grande échelle ».
- **Provenance à vérifier** : l'auteur dit avoir travaillé à partir d'une spécification publique qu'il a rédigée (`bitplane/rar-research`). Si une partie était dérivée du source UnRAR, la restriction UnRAR s'appliquerait, et `rars` sait aussi écrire des RAR, précisément ce que cette licence interdit. À lire avant de l'adopter (README, historique, issues).
- Le format RAR est propriétaire ; RARLAB tolère depuis toujours les décompresseurs libres. On n'utiliserait que la lecture : features `write`, `recovery`, `encryption` et `parallel` désactivées (`default-features = false`).
- Un décodeur jeune peut paniquer ou boucler sur une archive piégée : même traitement que les décodeurs d'image en phase 1 (`catch_unwind`, plafond de taille par entrée `MAX_ENTRY_SIZE`, plafond sur le total décompressé).

## 4. Intégration dans le code existant

- `shiori-core` a déjà un trait `Archive` (`entries`, `duplicates`, `open_entry`, `read_entry`) : `RarArchive` en serait une seconde implémentation, dans `archive/rar.rs`, derrière une feature Cargo `cbr`.
- Le scan d'un `.cbr` produit le même rapport et les mêmes codes ; `volumes.format = 'cbr'` (déjà prévu dans le schéma). Codes nouveaux possibles : `solid_archive` (info), `multivolume_archive` (error, les `.part2.rar` ne sont pas gérés), `unsupported_format` (option D ou feature désactivée).
- Avec la stratégie A, le scan écrit le zip converti puis enregistre les pages avec l'`archive_index` **du zip converti**. Les noms RAR (souvent en encodage OEM en RAR4) ne peuvent pas être réécrits octet pour octet par `zip` : les entrées converties sont nommées par ordinal (`000000.jpg`), le `page_id` reste calculé sur le nom brut RAR, et la vérification avant lecture porte sur le CRC et la taille au lieu du nom ; si celui-ci manque à la lecture, le serveur le reconstruit (ou renvoie une erreur explicite `conversion_missing` le temps de le refaire, jamais une autre page).

## 5. Recommandation

1. **Stratégie A** (conversion au scan en zip `stored` dans le dossier de données) : un seul chemin de lecture, des IDs inchangés, aucune latence à l'ouverture, et le coût de décompression est déjà payé par le scan d'intégrité. Le seul vrai coût est le disque ; la variante C le réduira si besoin.
2. **`rars` en lecture seule**, derrière une feature `cbr` (pur Rust, donc compatible avec le principe 9 même activée), **après** vérification de sa provenance et un essai sur de vrais CBR (RAR4 et RAR5, solid et non solid). Repli : libarchive derrière `cbr-libarchive` si `rars` échoue.
3. **En lot à part, en fin de phase 2** (lot 2.8), voire reporté : le reste de la phase 2 ne dépend pas du CBR.

## 6. Questions à trancher

- **Q1a** Gérer le CBR maintenant (lot 2.8), plus tard, ou jamais (option D) ? Combien de CBR as-tu, et de quelle taille au total ?
- **Q1b** Stratégie A (disque doublé pour les CBR, simple) ou B (disque borné, plus de code, première ouverture lente) ?
- **Q1c** `rars` (Rust pur, jeune, provenance à vérifier) ou libarchive (éprouvée, BSD, mais C et dépendance système) ?
- **Q1d** As-tu de vrais CBR à me donner pour l'essai (idéalement un RAR4 solid et un RAR5) ?
