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

![Accueil : « Continuer la lecture », puis les étagères des bibliothèques](/screens/fr/ecran_accueil.jpg)

- **Grand écran** : barre latérale (Accueil, Séries, Recherche, vos bibliothèques, Documentation, Téléchargés, Administration, Compte), repliable. Sur téléphone, la documentation s'ouvre depuis la page **Compte**.
- **Téléphone et tablette en portrait** : barre d'onglets en bas (Accueil, Séries, Recherche, Compte).
- **Accueil** : « Continuer la lecture » (le tome en cours de chaque série, ou le suivant quand un tome est fini), « Nouveautés » (les tomes ajoutés aux séries que vous suivez, jusqu'à ce que vous les ouvriez ou cliquiez « Tout marquer comme vu » ; absente quand il n'y en a pas), « À lire » (les séries que vous avez mises de côté, dans l'ordre d'ajout ; une série en part dès que vous la commencez), puis une étagère par bibliothèque.
- **Séries** : toutes les séries en couvertures, filtrables par lecture (non lu, en cours, lu), statut de publication, bibliothèque et **tags** (plusieurs tags cochés = séries qui les ont tous), triables par titre, dernière lecture ou ajout récent.
- **Recherche** : par titre, titre alternatif, auteur ou tag, sans tenir compte des accents ni des majuscules.
![Page Séries : recherche, filtres (lecture, publication, tags, bibliothèque), tri et couvertures](/screens/fr/ecran_series.jpg)


- **Page d'une série** : couverture, résumé, auteurs, genres et tags (chacun mène aux séries qui l'ont), avancement, bouton pour reprendre ou commencer, bouton **Suivre** (les tomes ajoutés ensuite apparaîtront dans « Nouveautés » ; ceux déjà là, non ; pour votre compte seulement, sur tous vos appareils), bouton **À lire** (proposé tant que la série n'est pas commencée), liste des tomes. Le menu d'un tome le marque lu ou non lu, le télécharge pour le hors-ligne et montre les problèmes de son dernier scan. Le menu de la série (sur sa page, ou sur sa carte dans les listes) la marque **entière** lue ou non lue, pour votre compte seulement ; « non lu » efface la progression de tous ses tomes et demande donc confirmation. Une série est « Lu » dès que tout ce qui se lit est lu : un tome illisible (en erreur) ne la retient pas, il reste signalé par son badge.

Une barre ambre en bas d'une couverture montre la progression d'un tome commencé.

![Page d'une série : couverture, progression, Reprendre, Modifier, Télécharger la série, Identifier, résumé et tags](/screens/fr/ecran_serie.jpg)

## Et ensuite

- [Ranger ses fichiers](/guide/libraries) pour que chaque série soit bien découpée.
- [Lire](/guide/reading), y compris hors ligne.
- [Créer les comptes de la famille](/guide/accounts).
- [Ouvrir Shiori à la tablette ou à Internet](/installation/network).
