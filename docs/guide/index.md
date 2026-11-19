# Premiers pas

Shiori lit vos dossiers de mangas, BD et webtoons et les sert à une application web que vous ouvrez dans le navigateur de votre tablette, téléphone ou ordinateur. Il ne modifie jamais vos fichiers.

Ce guide décrit Shiori écran par écran. Pour l'installer : [démarrage rapide](/installation/). Pour son fonctionnement interne : [documentation technique](/technical/).

## En cinq minutes

1. **Installez Shiori** ([Docker ou programme seul](/installation/)) et ouvrez <http://localhost:7878>.
2. **Créez le compte administrateur** sur la page **Installation**, avec le jeton d'installation affiché au démarrage.
3. **Administration → Ajouter une bibliothèque** : un nom, le dossier, le type (manga, BD, webtoon).
4. Le **scan** démarre tout seul ; les séries apparaissent sur l'accueil.
5. Ouvrez un tome et lisez. La page lue est enregistrée sur le serveur.

## L'interface

<!-- [ecran_accueil] -->

- **Grand écran** : barre latérale (Accueil, Séries, Recherche, vos bibliothèques, Documentation, Téléchargés, Administration, Compte), repliable. Sur téléphone, la documentation s'ouvre depuis la page **Compte**.
- **Téléphone et tablette en portrait** : barre d'onglets en bas (Accueil, Séries, Recherche, Compte).
- **Accueil** : « Continuer la lecture » (le tome en cours de chaque série, ou le suivant quand un tome est fini), puis une étagère par bibliothèque.
- **Séries** : toutes les séries en couvertures, filtrables par lecture (non lu, en cours, lu), statut de publication, bibliothèque et **tags** (plusieurs tags cochés = séries qui les ont tous), triables par titre, dernière lecture ou ajout récent.
- **Recherche** : par titre, titre alternatif, auteur ou tag, sans tenir compte des accents ni des majuscules.
<!-- [ecran_series] -->


- **Page d'une série** : couverture, résumé, auteurs, genres et tags (chacun mène aux séries qui l'ont), avancement, bouton pour reprendre ou commencer, liste des tomes. Le menu d'un tome le marque lu ou non lu, le télécharge pour le hors-ligne et montre les problèmes de son dernier scan.

Une barre ambre en bas d'une couverture montre la progression d'un tome commencé.

<!-- [ecran_serie] -->

## Et ensuite

- [Ranger ses fichiers](/guide/libraries) pour que chaque série soit bien découpée.
- [Lire](/guide/reading), y compris hors ligne.
- [Créer les comptes de la famille](/guide/accounts).
- [Ouvrir Shiori à la tablette ou à Internet](/installation/network).
