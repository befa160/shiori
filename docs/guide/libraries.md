# Bibliothèques et séries

## Bibliothèques

Une **bibliothèque** est un dossier de votre disque, avec un type qui donne le sens de lecture par défaut :

| Type | Sens de lecture par défaut |
|---|---|
| Manga | de droite à gauche, page par page |
| BD | de gauche à droite, page par page |
| Webtoon | défilement vertical continu |

Chaque **sous-dossier** de la bibliothèque est une **série**, et chaque fichier `.cbz`, `.cbr` ou `.pdf` qu'il contient est un **tome** (ou un chapitre). Les bibliothèques se créent dans **Administration → Ajouter une bibliothèque** ; deux bibliothèques ne peuvent pas se chevaucher.

Une bibliothèque neuve n'est visible que des administrateurs, sauf si **Donner accès à tous les lecteurs** est coché à sa création ; sinon, on l'ouvre aux comptes voulus dans **Administration → Comptes**.

Un tome est reconnu par son **contenu**, pas par son nom : renommer ou déplacer un fichier, ou renommer le dossier d'une série, ne perd ni la progression ni les métadonnées.

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

Des fichiers posés directement à la racine de la bibliothèque sont regroupés par nom de série ; ils rejoignent la série-dossier du même nom s'il y en a une.

## Formats

- **CBZ** : zip, y compris les très gros fichiers (zip64).
- **CBR** : RAR 4 et 5, y compris « solide » (la première page s'ouvre un peu plus lentement). Les archives en plusieurs volumes (`.part1.rar`) ne sont pas lues.
- Le type d'une archive est lu dans son contenu, pas dans son extension : un RAR nommé `.cbz` se lit. Une archive **7z** (CB7) n'est pas lue : elle est signalée (`unsupported_archive`), à reconvertir en CBZ.
- **PDF** : Shiori affiche les images incluses, **sans les marges** de la page PDF ; sur une tablette, la planche occupe tout l'écran. Une page faite de plusieurs morceaux (bandes, tuiles) est réassemblée. Une page de texte seul, ou dont l'image couvre moins de la moitié, est sautée et signalée ; le texte posé sur une planche (crédits, préface) n'est pas affiché. Un PDF protégé par un mot de passe n'est pas lu.
- **Images** : JPEG, PNG, WebP, AVIF, BMP, GIF (première image d'un GIF animé). Les pages sont triées dans l'ordre naturel de leurs noms (`2` avant `10`), quel que soit leur ordre dans l'archive.

## Ce que Shiori ne fait pas

- Il ne modifie, ne déplace et ne renomme **jamais** vos fichiers.
- Il ne suit pas les **liens symboliques** : montez le dossier lui-même (volume Docker, lien dur, montage réseau).
- Si le dossier d'une bibliothèque est absent, illisible ou soudain vide (disque réseau non monté), le scan échoue sans rien retirer : séries et réglages sont gardés jusqu'au retour des fichiers. Les tomes d'un sous-dossier illisible sont gardés tels quels, avec un avertissement au journal du scan.
