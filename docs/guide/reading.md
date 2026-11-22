# Lire

## Le lecteur

Le lecteur occupe tout l'écran. Les commandes s'affichent d'un **tap au centre** (ou avec Tab au clavier).

<!-- [ecran_lecteur] -->

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

<!-- [ecran_lecteur_affichage] -->


La page lue est enregistrée sur le serveur et suit d'un appareil à l'autre. En fin de tome, Shiori propose le tome suivant.

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

<!-- [ecran_telecharges] -->


Hors téléchargements, l'application garde aussi les pages déjà vues : revenir en arrière ou une courte coupure ne redemandent rien au réseau.

::: warning HTTPS
L'installation de l'application, la lecture hors ligne et les tomes téléchargés demandent une adresse **HTTPS** (ou `localhost`). Voir [Réseau et HTTPS](/installation/network).
:::

## Installer l'application

Shiori s'installe comme une application (PWA), en plein écran, sans la barre du navigateur :

- **Android (Chrome)** : menu ⋮ → **Installer l'application**.
- **iPad, iPhone (Safari)** : bouton Partager → **Sur l'écran d'accueil**.
