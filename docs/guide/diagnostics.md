# Codes de diagnostic

Chaque problème trouvé par un scan a un **code stable** et une **gravité**. Un tome avec au moins une **erreur** est marqué « Erreur » ; avec un **avertissement**, « Avertissement » ; une **information** ne change rien à l'affichage. Les problèmes d'un tome se lisent dans son menu → **Problèmes du scan**, et tous les fichiers concernés dans **Administration → Santé des bibliothèques**.

Une page en erreur n'est jamais remplacée par une autre : le lecteur affiche l'erreur à sa place, et le reste du tome reste lisible.

## Erreurs

| Code | Ce que ça veut dire | Que faire |
|---|---|---|
| `archive_unreadable` | Le fichier n'est pas un CBZ, un CBR ou un PDF valide, ou ne peut pas être lu (droits, disque). | Retélécharger ou recréer l'archive ; vérifier les droits. |
| `archive_truncated` | Le fichier est coupé (téléchargement ou copie incomplète). | Retélécharger ou recopier le fichier. |
| `multivolume_archive` | Archive en plusieurs volumes (`.part1.rar`…). | La réunir en une seule archive. |
| `encrypted_entry` | Une page, ou tout le PDF, est protégée par un mot de passe. | Recréer l'archive ou le PDF sans mot de passe. |
| `unsupported_compression` | Méthode de compression inconnue. | Recréer l'archive (zip standard). |
| `crc_mismatch` | Le contenu d'une page ne correspond pas à l'archive : page abîmée. | Retélécharger le fichier. |
| `size_mismatch` | La taille d'une page ne correspond pas à l'archive : page abîmée. | Retélécharger le fichier. |
| `entry_too_large` | Une page est trop grande pour être lue sans risque. | Réduire l'image et recréer l'archive. |
| `image_undecodable` | Une image ne peut pas être décodée (ou, dans un PDF, annonce trop de pixels). | Remplacer l'image abîmée. |
| `duplicate_entry_conflict` | Deux pages portent le même nom avec des contenus différents. | Renommer l'une des deux dans l'archive. |
| `no_pages` | Aucune page lisible dans le fichier. | Vérifier le contenu de l'archive. |
| `file_changed_during_scan` | Le fichier a changé pendant sa lecture (copie en cours). | Relancer le scan une fois la copie finie (la surveillance le fait seule). |

## Avertissements

| Code | Ce que ça veut dire |
|---|---|
| `duplicate_entry_identical` | Une page est en double avec un contenu identique ; une seule est gardée. |
| `extension_mismatch` | L'extension ne correspond pas au format réel de l'image (un `.jpg` qui est un PNG) ; la page est lue quand même. |
| `unsupported_image` | Image dans un format non pris en charge (GIF, BMP, TIFF, JPEG XL, HEIC) : page ignorée. |
| `non_image_entry` | Un fichier qui n'est pas une image : ignoré. |
| `subdirectories` | Les pages sont rangées dans des sous-dossiers de l'archive ; elles sont lues quand même. |
| `archive_order_differs` | L'ordre des pages dans l'archive diffère de l'ordre de lecture ; Shiori suit l'ordre des noms. |
| `metadata_invalid` | Un `ComicInfo.xml` ou un `index.json` illisible : ignoré, le tome reste lisible. |
| `pdf_page_without_image` | Une page du PDF sans image (texte seul), ou dont l'image couvre moins de la moitié : sautée. |

## Informations

| Code | Ce que ça veut dire |
|---|---|
| `ignored_entry` | Un fichier annexe connu (`ComicInfo.xml`, `Thumbs.db`, `.DS_Store`, dossier `__MACOSX`) a été ignoré. |
| `solid_archive` | Archive RAR solide : la première page met un peu plus de temps à s'ouvrir. |
| `pdf_text_ignored` | Le texte visible d'une page du PDF n'est pas affiché : seule l'image l'est. |
