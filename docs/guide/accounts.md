# Comptes et sécurité

Chaque personne se connecte avec son compte : sa progression, ses réglages, les bibliothèques qui lui sont ouvertes.

| Rôle | Peut |
|---|---|
| **Administrateur** | tout : bibliothèques, scans, métadonnées (communes à tous), comptes, réglages par défaut, sauvegardes ; voit toutes les bibliothèques |
| **Lecteur** | lire les bibliothèques qui lui sont ouvertes, avec sa progression, ses réglages de lecture et ses préférences |

## Créer les comptes

- **Premier compte** : page **Installation** tant qu'aucun compte n'existe, avec le **jeton d'installation** affiché au démarrage de Shiori (il change à chaque lancement et empêche le premier venu de votre réseau de prendre l'instance). Sans interface : `shiori user add <nom> --admin`.
- **Administration → Comptes** : créer un compte (nom, rôle, bibliothèques ouvertes), modifier son rôle et ses bibliothèques, le désactiver, lui donner un nouveau mot de passe provisoire, déconnecter tous ses appareils et jetons, retirer sa double authentification, le supprimer (sa progression part avec lui, pas les fichiers ni les fiches).
- Un **mot de passe provisoire** est montré une seule fois et doit être changé à la première connexion.
- Le dernier administrateur actif ne peut être ni rétrogradé, ni désactivé, ni supprimé.

## Se connecter

Nom et mot de passe (8 caractères au moins), puis le code de la double authentification si elle est active. Après plusieurs mots de passe faux, Shiori fait patienter avant le prochain essai, de plus en plus longtemps (15 minutes au plus), par compte et par adresse. Une session dure **90 jours après le dernier usage** : la tablette reste connectée tant qu'on s'en sert.

## Page Compte

<!-- [ecran_compte] -->

- **Préférences** : langue, thème et sens des pages de ce compte, sur tous ses appareils.
- **Mot de passe** : le changer déconnecte ses autres appareils.
- **Appareils connectés** : chaque session avec son dernier usage ; déconnecter un appareil perdu.
- **Double authentification** : en plus du mot de passe, un code à 6 chiffres d'une application d'authentification (Aegis, Google Authenticator, 2FAS…). L'activation demande le mot de passe, montre un QR code (ou une clé à saisir), vérifie un premier code et donne **10 codes de secours**, montrés une seule fois, qui remplacent chacun une fois le code du téléphone. Conseillée si Shiori est ouvert sur Internet.
- **Jetons d'API** : pour qu'un autre logiciel lise les bibliothèques du compte et enregistre sa progression sans le mot de passe. Votre mot de passe est demandé pour en créer un. Un jeton est montré une seule fois, survit au changement de mot de passe (le supprimer, ou « déconnecter tous les appareils »), peut expirer (30 jours, 90 jours, 1 an ou jamais) et ne permet que la lecture et la progression. Voir [l'API](/api/).
- **Se déconnecter** : ferme la session de cet appareil et efface les pages gardées en cache ; propose aussi d'effacer les tomes téléchargés.

## Oublis et pertes

- **Mot de passe oublié** : l'administrateur en donne un nouveau dans **Administration → Comptes**. Pour l'administrateur lui-même : `shiori user password <nom>`.
- **Téléphone perdu** : un code de secours, sinon un administrateur retire la double authentification (**Administration → Comptes**, ou `shiori user totp-reset <nom>`).
