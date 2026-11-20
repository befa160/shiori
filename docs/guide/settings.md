# Réglages

## Apparence et langue

- **Thème** : Sombre (par défaut), Clair, Noir OLED, Sépia, ou Système (sombre ou clair selon l'appareil). Le lecteur reste toujours sombre.
- **Langue** : français ou anglais ; sans choix, celle du navigateur.

Chaque compte choisit les siens dans **Compte → Préférences** (la langue aussi dans la barre latérale), enregistrés sur le serveur pour tous ses appareils. L'administration fixe ceux des comptes qui n'ont rien choisi (**Administration → Langue de l'interface**, **Thème**, **Lecture**).

## Administration

| Section | Réglage |
|---|---|
| Bibliothèques | ajout, scan, suppression (les fichiers ne sont jamais touchés), surveillance des dossiers, intervalle du scan complet |
| Santé des bibliothèques | état des tomes, vérification en profondeur, fichiers à réparer |
| Historique et options | scan en cours, lancement avec options, historique et journaux |
| Comptes | rôles, bibliothèques ouvertes, mots de passe, double authentification |
| Lecture | sens par défaut pour tourner les pages |
| Métadonnées web | autorisation, ordre des sources, fiches pour adultes, identification |
| Langue, Thème | défauts des comptes |
| Données | emplacement de la base et du cache, sauvegardes |

## Sauvegardes

Shiori sauvegarde sa base chaque jour (les 7 plus récentes gardées), avant chaque mise à jour de son format et avant chaque restauration. **Administration → Données** montre le dossier des sauvegardes et la liste, et permet de sauvegarder tout de suite, de télécharger, de supprimer ou de **restaurer** une sauvegarde, depuis la liste ou un fichier, sans arrêter Shiori. Après une restauration, tout le monde se reconnecte. Détails : [Sauvegardes](/installation/backup).

## Limites actuelles

- Les comptes se créent par l'administrateur seulement : pas encore de lien d'invitation, ni de connexion déléguée à un proxy d'authentification (en-tête `Remote-User`).
- Une seule adresse par installation de l'application : pour passer par l'adresse locale à la maison et l'adresse publique ailleurs, voir le [DNS local](/installation/network#la-meme-adresse-a-la-maison-et-dehors).
- Les métadonnées web viennent d'AniList, de MangaDex et de Wikipédia seulement.
