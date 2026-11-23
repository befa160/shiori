# Lire

## Le lecteur

Le lecteur occupe tout l'écran. Les commandes s'affichent d'un **tap au centre** (ou avec Tab au clavier).

![Lecteur plein écran, commandes affichées : retour, titre, Affichage et curseur des pages](/screens/fr/ecran_lecteur.jpg)

| Geste | Effet |
|---|---|
| Tap à gauche ou à droite, balayage | page précédente ou suivante, selon le sens choisi |
| Flèches, Page suivante, Espace, Début, Fin | idem au clavier |
| Pincement (au doigt) | zoom du navigateur, progressif ; on s'y déplace au doigt, et aucun geste ne tourne la page tant qu'elle est agrandie |
| Double-clic au centre (à la souris) | zoom ×2 à l'endroit cliqué ; on s'y déplace en glissant, un clic ne tourne pas la page ; double-clic, Échap ou page tournée pour en sortir |
| Curseur des commandes | aller à n'importe quelle page |

Le panneau **Affichage** règle, pour la série :

- le **sens de lecture** (manga, BD, webtoon) ;
- la **disposition** : page simple, double page, ou automatique (double en paysage) ; en page simple, une double page est coupée en deux moitiés lues dans l'ordre ;
- le sens pour **tourner les pages** (« comme le sens de lecture », ou toucher à droite / à gauche pour avancer), avec un défaut général dans l'administration et dans les préférences du compte.

![Panneau Affichage du lecteur : sens de lecture, tourner les pages, disposition, doubles pages](/screens/fr/ecran_lecteur_affichage.jpg)


La page lue est enregistrée sur le serveur et suit d'un appareil à l'autre. En fin de tome, Shiori propose le tome suivant ; s'il est en erreur au dernier scan, le bouton le dit (« Tome suivant (en erreur) »), et le tome affiche ses pages illisibles en erreur.

Une page abîmée dans l'archive affiche une **erreur explicite à sa place**, jamais une autre image. Si le fichier a changé depuis le scan, le lecteur le dit et demande un nouveau scan.

## Pages adaptées à l'écran

Les pages sont servies à la largeur de l'écran (en pixels réels), en AVIF, et gardées en cache sur le serveur et sur l'appareil. Une page plus étroite que l'écran est servie telle quelle, jamais agrandie. Les quatre pages suivantes sont préchargées (deux sur une connexion lente ou en économie de données), et le serveur prépare d'avance les cinq suivantes : feuilleter vite ne fait plus attendre l'encodage de chaque page.

## Lire hors ligne

Un tome peut être **gardé sur l'appareil** pour lire sans réseau. Depuis la page d'une série : menu d'un tome → **Télécharger pour lire hors ligne**, ou **Télécharger la série** pour tous les tomes qui manquent.

- Les pages sont téléchargées une par une, à la taille de l'écran de cet appareil : un tome pèse nettement moins que son archive.
- **Tomes téléchargés** (barre latérale, administration, ou le lien de l'avis « Hors ligne ») liste les tomes gardés, leur taille et la place disponible ; c'est par là qu'on les ouvre sans réseau.
- La page lue hors ligne est envoyée au serveur dès le retour du réseau, avec son heure : elle ne remplace pas une page lue depuis sur un autre appareil.
- Un téléchargement interrompu (application fermée) reprend au lancement suivant ; une coupure de réseau l'arrête, à relancer.
- Installez l'application **depuis le navigateur où vous lisez, à l'adresse habituelle**, et ouvrez-la une fois en ligne avant de partir. Une autre adresse ou un autre navigateur (Samsung Internet au lieu de Chrome, par exemple), c'est une autre application, avec son propre stockage. Si la lecture hors ligne ne peut pas marcher sur cette installation, **Tomes téléchargés** le dit et le téléchargement est refusé.

![Page Tomes téléchargés : place utilisée, téléchargement en cours et tome gardé](/screens/fr/ecran_telecharges.jpg)


Hors téléchargements, l'application garde aussi les pages déjà vues : revenir en arrière ou une courte coupure ne redemandent rien au réseau.

::: warning HTTPS
L'installation de l'application, la lecture hors ligne et les tomes téléchargés demandent une adresse **HTTPS** (ou `localhost`). Voir [Réseau et HTTPS](/installation/network).
:::

## Statistiques de lecture

**Statistiques** (barre latérale, ou page **Compte** sur téléphone et tablette) montre ce que **votre compte** a lu : personne d'autre ne les voit, pas même l'administrateur.

- **Totaux depuis le début** : pages lues, temps de lecture, tomes terminés, séries terminées (tous leurs tomes lisibles lus).
- **12 dernières semaines** : les pages lues chaque semaine (du lundi au dimanche), avec le temps et les tomes terminés ; survolez une colonne pour son détail, ou ouvrez **Détail par semaine** pour le tableau.
- Les jours suivent le fuseau de l'appareil : une lecture à 23 h 30 compte pour ce jour-là.
- Une avance de plus de 50 pages d'un coup (le curseur) ne compte pas comme des pages lues, sauf une lecture hors ligne envoyée au retour du réseau ; une pause de 5 minutes ou plus ne compte pas dans le temps.
- **Marquer comme lu** n'entre pas dans les statistiques : ce n'est pas une lecture.
- Avant la version 1.1, seuls les tomes terminés sont connus : ils comptent le jour de leur fin, sans pages ni temps.

## Installer l'application

Shiori s'installe comme une application (PWA), en plein écran, sans la barre du navigateur :

- **Android (Chrome)** : menu ⋮ → **Installer l'application**.
- **iPad, iPhone (Safari)** : bouton Partager → **Sur l'écran d'accueil**.
