# Archives et scan

Chaque format implémente le trait `Archive` de `shiori-core` : liste des entrées dans l'ordre de l'archive, lecture d'une entrée par son index, vérification du CRC et de la taille. Le format se choisit d'après les premiers octets du fichier, jamais d'après l'extension : un zip renommé `.cbr` reste un zip.

## CBZ

Crate `zip` : lecture d'une entrée par l'annuaire central, sans extraction complète ; zip64 et gros fichiers gérés. Chaque page est relue contre le CRC et la taille de l'annuaire.

## CBR

Crate `rars` (Rust pur, jamais `unrar`) : RAR 4 et 5.

- **Non solide** : lecture directe d'un membre par son index, mêmes vérifications que le CBZ.
- **Solide** (pas d'accès aléatoire) : à la première demande d'un tome, une passe extrait toutes ses entrées dans `<cache>/extract/<volume_id>/<index>` (dossier temporaire renommé à la fin : jamais de dossier partiel visible). Chaque fichier est relu contre la taille du scan avant d'être servi. Une seule extraction à la fois. Les tomes extraits partagent le plafond `--cache-max-mib` avec les rendus ; le moins récemment lu part en premier, jamais le dernier tome lu.
- **Limites** : 1 000 000 en-têtes au plus, sortie bornée à la somme des tailles déclarées, chaque page dans un tampon borné à sa taille déclarée ; une entrée trop grande n'est jamais mise en mémoire (`entry_too_large`). Les archives en plusieurs volumes sont refusées (`multivolume_archive`), un RAR coupé aussi (`archive_truncated`).
- **Doublons de nom** : le RAR les garde tous ; seule la dernière occurrence fait une page.

## PDF

Crate `hayro-interpret` (Rust pur), lu comme une archive, sans conversion : Shiori sert les **images incluses**, pas la page PDF avec ses marges.

- Un `Device` qui ne dessine rien interprète chaque page (matrices, formulaires, `/Rotate`, couleurs, transparences) et relève ses images et son texte.
- Une page dont les images couvrent au moins la moitié de la surface donne une entrée `NNNN.jpg` ou `NNNN.png` (numéro de la page PDF) ; les autres sont écartées avec `pdf_page_without_image`.
- Un JPEG droit, gris ou RVB, sans masque, est servi **tel quel** : sa position dans le fichier est relevée à l'ouverture, puis lue directement (CRC revérifié).
- Toute autre page est décodée puis encodée en PNG : image redressée, transparence gardée, page faite de plusieurs images (bandes, tuiles, calques) **recomposée** à la densité de l'image la plus fine et rognée à l'union des images.
- Garde-fous : `%%EOF` exigé (sinon `archive_truncated`), 128 Mpx au plus par page, morceaux de moins d'un millième de la page écartés. Un PDF chiffré est une erreur (`encrypted_entry`).

## Scan d'intégrité

`shiori-core::integrity::scan_file` produit un rapport par fichier :

- CRC et taille de chaque entrée, en-têtes et dimensions des images (`imagesize`, sans décoder), doublons, fichiers parasites, sous-dossiers, ordre, archives tronquées ou chiffrées ; `--deep` décode chaque image.
- Chaque problème a un code stable et une gravité ([codes de diagnostic](/guide/diagnostics)). Une page illisible est marquée `broken` ; le tome reste lisible pour le reste.
- Une panique dans une bibliothèque tierce (archive malformée) devient un problème du fichier, jamais l'arrêt du scan.
- Un fichier modifié pendant sa lecture n'a pas de tome fiable (`file_changed_during_scan`) : rien n'est rattaché, il sera relu.
- **Métadonnées des fichiers** : `ComicInfo.xml` et `index.json` lus pendant le scan (RAR solide compris), et le dictionnaire `Info` d'un PDF (auteurs, sujet → résumé, mots-clés → tags ; ni titre ni date) ; un fichier illisible est un avertissement, jamais une erreur de tome.

Le rescan évite le hash : un fichier dont la taille et la date n'ont pas changé est compté « inchangé ». `SCANNER_VERSION` (règles du scan) et `PARSER_VERSION` (parseur de noms), figées par des tests, relancent l'analyse quand elles changent.

## Parseur de noms

`shiori-core::name::parse(chemin relatif)` est une fonction pure : série = premier dossier sous la racine, puis type (tome, chapitre, spécial, inconnu), numéro et fin d'intervalle d'après des marqueurs français, anglais et japonais. Règles et exemples : [Ranger ses fichiers](/guide/libraries#ranger-ses-fichiers).
