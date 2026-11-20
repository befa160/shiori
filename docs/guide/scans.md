# Scans, surveillance et santé

## Le scan

Un scan lit les fichiers d'une bibliothèque, vérifie chaque page (taille, CRC, en-tête, dimensions) et met la base à jour. C'est la seule source de vérité : le lecteur n'affiche que ce que le scan a établi.

Il démarre à la création d'une bibliothèque, quand un fichier change (voir la surveillance ci-dessous), et à la demande : bouton à côté de chaque bibliothèque dans la barre latérale, ou **Administration → Historique et options**, avec deux options :

- **Contrôle approfondi** : décode entièrement chaque image (lent, mais trouve les images abîmées) ;
- **Relire tous les fichiers** : relit même les fichiers que Shiori croit inchangés (d'ordinaire, taille et date suffisent).

Un premier scan lit chaque fichier en entier ; les suivants ne relisent que ce qui a changé (taille ou date du fichier). Un seul scan tourne à la fois.

Ordre de grandeur du premier scan, sur une vraie bibliothèque : **4 941 tomes (907 Go) en environ 55 minutes** sur un NAS (Celeron N5105, disques durs), limité par la lecture des disques. Une nouvelle version qui change les règles du scan relit tout une fois ; ses notes de version le disent.

## Surveillance des dossiers

Les dossiers de chaque bibliothèque sont **surveillés** : un fichier ajouté, remplacé ou retiré lance, **30 secondes** après le dernier changement (le temps qu'une copie finisse), un scan du seul dossier de série touché.

- Au démarrage, chaque bibliothèque surveillée est rescannée pour rattraper ce qui a changé pendant que Shiori était éteint.
- Un **scan complet** de toutes les bibliothèques passe en plus une fois par jour (réglable : toutes les heures, toutes les 6 heures, jamais). Sur un partage réseau (SMB, NFS) ou avec Docker Desktop, les changements faits depuis une autre machine ne sont pas signalés ; ce scan les rattrape.
- Si le dossier d'une bibliothèque disparaît (disque débranché, partage démonté), rien n'est retiré de la bibliothèque.

<!-- [ecran_admin_bibliotheques] -->


La surveillance se coupe par bibliothèque (**Administration → Bibliothèques**, interrupteur « Surveiller les dossiers ») ; l'intervalle du scan complet se règle au même endroit.

## Historique et journaux

<!-- [ecran_scans] -->

**Administration → Historique et options** montre le scan en cours (avancement, fichier lu, annulation) et l'historique des passes, filtrable par statut et par bibliothèque. Chaque passe garde son journal (fichiers lus, problèmes, erreurs) avec un lien vers chaque tome concerné, et dit d'où elle vient : vous, la surveillance des dossiers ou le scan complet périodique. Les passes sont gardées 30 jours (option `--scan-log-days`).

## Santé des bibliothèques

<!-- [ecran_sante] -->

**Administration → Santé des bibliothèques** fait le point :

- pour chaque bibliothèque, le nombre de tomes **sains**, en **avertissement**, en **erreur** et de fichiers **illisibles** ;
- la date de la dernière **vérification complète**, et un bouton **Vérifier en profondeur** (contrôle approfondi de toute la bibliothèque, seul à mettre cette date à jour) ;
- la liste des fichiers **à réparer**, filtrable par bibliothèque et par problème, avec chaque problème expliqué ([codes de diagnostic](/guide/diagnostics)) et un lien vers le tome.

## En ligne de commande

La même vérification existe sans serveur :

```bash
shiori scan /srv/mangas --deep
```

Options : `--deep` (contrôle approfondi), `--rehash` (relire même les fichiers inchangés), `--jobs N` (fichiers en parallèle), `--json <fichier|->` (rapport JSON complet, `-` pour la sortie standard), `--db <fichier>` ou `--no-db` (sans base, rapport seul), `--fail-on error|warning` (gravité qui donne le code de sortie 1, `error` par défaut). Code de sortie : 0 ok, 1 problème au-delà de `--fail-on`, 2 erreur d'usage ou fatale. Aide : `shiori scan --help`.
