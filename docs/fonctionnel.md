# Utiliser Shiori

Ce que fait Shiori, écran par écran. Pour l'installer : [installation](installation.fr.md). Pour son fonctionnement interne : [documentation technique](technique.md).

## Bibliothèques et séries

Une **bibliothèque** est un dossier de votre disque, avec un type qui donne le sens de lecture par défaut :

| Type | Sens de lecture par défaut |
|---|---|
| Manga | de droite à gauche, page par page |
| BD | de gauche à droite, page par page |
| Webtoon | défilement vertical continu |

Chaque **sous-dossier** de la bibliothèque est une **série**, et chaque fichier `.cbz` ou `.cbr` qu'il contient est un **tome** (ou un chapitre). Les bibliothèques se créent dans **Administration → Ajouter une bibliothèque** ; deux bibliothèques ne peuvent pas se chevaucher.

Un tome est reconnu par son contenu, pas par son nom : renommer ou déplacer un fichier, ou renommer le dossier d'une série, ne perd ni la progression ni les métadonnées.

## Ranger ses fichiers

```
Mangas/                         ← la bibliothèque
├── One Piece/                  ← une série par dossier
│   ├── One Piece T01.cbz
│   ├── One Piece T02.cbz
│   └── One Piece HS 01.cbz     ← hors-série, classé après les tomes
└── Blame!/
    ├── Blame! v01.cbr
    └── Blame! v02.cbr
```

Shiori lit le **numéro** dans le nom du fichier. Il reconnaît, sans tenir compte des majuscules, avec ou sans point ni espace :

- **tomes** : `Tome 3`, `T03`, `Vol 3`, `Volume 3`, `v03`, `Intégrale`, `第3巻` ;
- **chapitres** : `Chapitre 12`, `Ch 12`, `Chapter 12`, `c012`, `#12`, `第12話` ;
- **spéciaux** : `HS`, `Hors-série`, `Special`, `Extra`, `Omake`, `One-shot` ;
- **numéros** : `03` ou `3`, décimaux (`12.5`, `12,5`), intervalles (`01-03`, `01 à 03`) ; à défaut, un nombre seul en fin de nom (`Blame 03.cbz` → tome 3).

Ce qui est entre crochets ou parenthèses est ignoré (`[Team]`, `(2019)`, `(Digital)`), et une année seule n'est jamais prise pour un numéro. Dans une série, l'ordre est : tomes, puis chapitres hors tome, puis spéciaux, puis les fichiers sans numéro reconnu, chacun par numéro puis par nom.

Formats lus : **CBZ** (zip, y compris les très gros fichiers) et **CBR** (RAR 4 et 5, y compris « solide », dont la première page s'ouvre un peu plus lentement). Images : JPEG, PNG, WebP, AVIF. Les pages sont triées dans l'ordre naturel de leurs noms (`2` avant `10`), quel que soit leur ordre dans l'archive. Shiori ne modifie jamais vos fichiers.

## Accueil, séries, recherche

- **Accueil** : « Continuer la lecture » (le tome en cours de chaque série, ou le suivant quand un tome est fini), puis une étagère par bibliothèque.
- **Séries** : toutes les séries en couvertures, avec des filtres par lecture (non lu, en cours, lu), par statut de publication et par bibliothèque, et un tri (par titre, dernière lecture, récemment ajoutées).
- **Recherche** : par titre, titre alternatif ou auteur, sans tenir compte des accents ni des majuscules.
- **Page d'une série** : couverture, résumé, auteurs, genres, avancement en pages, bouton pour reprendre ou commencer, et la liste des tomes. Le menu d'un tome permet de le marquer lu ou non lu, de le télécharger pour le hors-ligne et de voir les problèmes de son dernier scan.

Une barre ambre en bas d'une couverture montre la progression d'un tome commencé.

## Lire

Le lecteur occupe tout l'écran. Les commandes s'affichent d'un **tap au centre** (ou avec Tab au clavier).

| Geste | Effet |
|---|---|
| Tap à gauche ou à droite, balayage | page précédente ou suivante, selon le sens choisi |
| Flèches, Page suivante, Espace, Début, Fin | idem au clavier |
| Double tap | zoom ×2 à l'endroit touché ; tourner la page le quitte |
| Curseur des commandes | aller à n'importe quelle page |

Le panneau **Affichage** règle, pour la série :

- le **sens de lecture** (manga, BD, webtoon) ;
- la **disposition** : page simple, double page, ou automatique (double en paysage) ; en page simple, une double page est coupée en deux moitiés lues dans l'ordre ;
- le sens pour **tourner les pages** (« comme le sens de lecture », ou toucher à droite / à gauche pour avancer), avec un défaut général dans l'administration.

La page lue est enregistrée sur le serveur et suit d'un appareil à l'autre. Une page abîmée dans l'archive affiche une **erreur explicite à sa place**, jamais une autre image. En fin de tome, Shiori propose le tome suivant.

## Lire hors ligne

Un tome peut être **gardé sur l'appareil** pour lire sans réseau (dans le train, en vacances). Depuis la page d'une série : menu d'un tome → **Télécharger pour lire hors ligne**, ou **Télécharger la série** pour tous les tomes qui manquent.

- Les pages sont téléchargées une par une, à la taille de l'écran de cet appareil : un tome pèse nettement moins que son archive.
- **Tomes téléchargés** (barre latérale, administration, ou le lien de l'avis « Hors ligne ») liste les tomes gardés, leur taille et la place disponible ; c'est par là qu'on les ouvre sans réseau.
- La page lue hors ligne est envoyée au serveur dès le retour du réseau.
- Un téléchargement interrompu (application fermée) reprend au lancement suivant ; une coupure de réseau pendant un téléchargement l'arrête, à relancer.

Hors téléchargements, l'application garde aussi les pages déjà vues : revenir en arrière ou une courte coupure ne redemandent rien au réseau. La lecture hors ligne demande une adresse HTTPS (voir l'[installation](installation.fr.md#installer-lapplication-sur-la-tablette-ou-le-téléphone)).

## Métadonnées

Les informations d'une série (titre, titres alternatifs, résumé, auteurs, genres, année, éditeur, statut de publication, couverture, liens) viennent de trois sources, la première qui répond l'emporte, **champ par champ** :

1. **vos modifications** (bouton **Modifier** de la page série) : un champ modifié est verrouillé, et ni un rescan ni une source web ne le remplacent ; « Rétablir » le rend aux sources ;
2. **vos fichiers** : `ComicInfo.xml` et `index.json` dans les archives, lus au scan ;
3. **les sources web**, si vous les autorisez : AniList et MangaDex.

Le **titre affiché** est le titre modifié, sinon celui des fichiers, sinon celui des sources web (dans la langue de l'interface, ou en romaji), sinon le nom du dossier. Les autres titres restent cherchables.

Les **sources web** sont désactivées par défaut (**Administration → Métadonnées web**). Seul le titre de la série part, avec un nom de client générique ; jamais un chemin, un nom de fichier ou un nom d'utilisateur. Une fois autorisées :

- après chaque scan, une série est liée toute seule si un seul résultat porte exactement son titre ; sinon elle est « à vérifier » ;
- **Identifier** (page série) cherche dans une source et propose les résultats avec leur couverture ;
- l'ordre des sources se règle dans l'administration (par défaut, MangaDex d'abord en français, AniList d'abord en anglais), et une série peut imposer sa source principale ;
- les fiches pour adultes sont exclues des recherches, sauf si l'administration les permet.

La **couverture** d'une série est la première page de son premier tome, ou celle que vous choisissez : n'importe quelle page, ou la couverture d'une source (gardée par le serveur).

## Scans et journaux

Un scan lit les fichiers d'une bibliothèque, vérifie chaque page et met la base à jour. Il démarre à la création d'une bibliothèque, et à la demande : bouton à côté de chaque bibliothèque dans la barre latérale, ou **Administration → Historique et options**, avec deux options :

- **Contrôle approfondi** : décode entièrement chaque image (lent) ;
- **Relire tous les fichiers** : relit même les fichiers que Shiori croit inchangés (d'ordinaire, taille et date suffisent).

Le scan en cours s'affiche avec son avancement et s'annule. L'historique garde chaque passe 30 jours (option `--scan-log-days`), avec son journal (fichiers lus, problèmes, erreurs) et un lien vers chaque tome concerné. Un tome dont le fichier a changé est relu au scan suivant ; un tome dont le fichier a disparu le signale.

La même vérification existe en ligne de commande, sans serveur : `shiori scan <dossier>` (aide : `shiori scan --help`).

## Codes de diagnostic

Chaque problème trouvé par un scan a un code stable et une gravité. Un tome avec au moins une **erreur** est marqué « Erreur » ; avec un **avertissement**, « Avertissement » ; une **information** ne change rien à l'affichage. Les problèmes d'un tome se lisent dans son menu → **Problèmes du scan**.

### Erreurs

| Code | Ce que ça veut dire | Que faire |
|---|---|---|
| `archive_unreadable` | Le fichier n'est pas un CBZ ou un CBR valide. | Retélécharger ou recréer l'archive. |
| `archive_truncated` | Le fichier est coupé (téléchargement ou copie incomplète). | Retélécharger ou recopier le fichier. |
| `multivolume_archive` | Archive en plusieurs volumes (`.part1.rar`…). | La réunir en une seule archive. |
| `encrypted_entry` | Une page est protégée par un mot de passe. | Recréer l'archive sans mot de passe. |
| `unsupported_compression` | Méthode de compression inconnue. | Recréer l'archive (zip standard). |
| `crc_mismatch` | Le contenu d'une page ne correspond pas à l'archive : page abîmée. | Retélécharger le fichier. |
| `size_mismatch` | La taille d'une page ne correspond pas à l'archive : page abîmée. | Retélécharger le fichier. |
| `entry_too_large` | Une page est trop grande pour être lue sans risque. | Réduire l'image et recréer l'archive. |
| `image_undecodable` | Une image ne peut pas être décodée. | Remplacer l'image abîmée. |
| `duplicate_entry_conflict` | Deux pages portent le même nom avec des contenus différents. | Renommer l'une des deux dans l'archive. |
| `no_pages` | Aucune page lisible dans le fichier. | Vérifier le contenu de l'archive. |
| `file_changed_during_scan` | Le fichier a changé pendant sa lecture (copie en cours). | Relancer le scan une fois la copie finie. |

### Avertissements

| Code | Ce que ça veut dire |
|---|---|
| `duplicate_entry_identical` | Une page est en double avec un contenu identique ; une seule est gardée. |
| `extension_mismatch` | L'extension ne correspond pas au format réel de l'image (un `.jpg` qui est un PNG) ; la page est lue quand même. |
| `unsupported_image` | Image dans un format non pris en charge (GIF, BMP, TIFF, JPEG XL, HEIC) : page ignorée. |
| `non_image_entry` | Un fichier qui n'est pas une image : ignoré. |
| `subdirectories` | Les pages sont rangées dans des sous-dossiers de l'archive ; elles sont lues quand même. |
| `archive_order_differs` | L'ordre des pages dans l'archive diffère de l'ordre de lecture ; Shiori suit l'ordre des noms. |
| `metadata_invalid` | Un `ComicInfo.xml` ou un `index.json` illisible : ignoré, le tome reste lisible. |

### Informations

| Code | Ce que ça veut dire |
|---|---|
| `ignored_entry` | Un fichier annexe connu (`ComicInfo.xml`, `Thumbs.db`, `.DS_Store`, dossier `__MACOSX`) a été ignoré. |
| `solid_archive` | Archive RAR solide : la première page met un peu plus de temps à s'ouvrir. |

## Apparence et langue

- **Thème** (**Administration → Thème**) : Sombre (par défaut), Clair, Noir OLED, Sépia, ou Système (sombre ou clair selon l'appareil). Le lecteur reste toujours sombre.
- **Langue** : français ou anglais, choisie dans la barre latérale ou l'administration ; sans choix, celle du navigateur.

Thème et langue sont enregistrés sur le serveur et s'appliquent à tous les appareils.

## Limites actuelles

- **Pas de comptes ni de mot de passe** : la progression et les réglages sont communs, et l'accès doit être protégé par le réseau ou un reverse proxy. Les comptes arrivent en phase 7 ([ROADMAP](../ROADMAP.md)).
- Une seule adresse par installation de l'application : pour passer par l'adresse locale à la maison et l'adresse publique ailleurs, voir le [DNS local](installation.fr.md#la-même-adresse-à-la-maison-et-dehors).
- Les métadonnées web viennent d'AniList et de MangaDex seulement.
