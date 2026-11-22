# Démarrage rapide

Shiori est un seul programme : il lit vos dossiers de mangas, BD et webtoons et les sert à une application web. Deux façons de l'installer :

- **[Docker](#avec-docker)** : le plus simple sur un NAS, un serveur ou un Raspberry Pi.
- **[Le programme seul](#avec-le-programme-seul)** : Linux, Windows ou macOS, sans rien d'autre à installer.

Par défaut, Shiori n'est joignable que depuis l'appareil où il tourne ; pour l'ouvrir à votre réseau ou à Internet, voir [Réseau et HTTPS](/installation/network).

## Avec Docker

Il faut Docker et Docker Compose ([installer Docker](https://docs.docker.com/get-started/get-docker/)).

1. Créez un dossier `shiori` et placez-y ce `compose.yaml` (le même que dans [le dépôt](https://github.com/befa160/shiori/blob/main/compose.yaml)) :

   ```yaml
   services:
     shiori:
       image: ghcr.io/befa160/shiori:latest
       restart: unless-stopped
       ports:
         - "127.0.0.1:7878:7878"   # « 7878:7878 » pour le réseau local
       environment:
         SHIORI_ALLOWED_HOSTS: localhost
       volumes:
         - shiori-data:/data     # base, sauvegardes dans /data/backups
         - shiori-cache:/cache   # pages redimensionnées, se reconstruit
         - /srv/mangas:/bibliotheques/mangas:ro

   volumes:
     shiori-data:
     shiori-cache:
   ```

   Remplacez `/srv/mangas` par le vrai dossier. La partie de droite (`/bibliotheques/mangas`) est le chemin vu par Shiori : c'est celui que vous donnerez dans l'interface. `:ro` le rend en lecture seule. Ajoutez une ligne par dossier (BD, webtoons…).

2. Lancez Shiori :

   ```bash
   docker compose up -d
   ```

3. Ouvrez <http://localhost:7878> et passez aux [premiers pas](#premiers-pas).

L'image (`distroless`, amd64 et arm64) tourne sans droits d'administrateur. Gardez les **volumes nommés** : un dossier de l'hôte créé par Docker appartient à root, et Shiori ne pourrait pas y écrire.

## Avec le programme seul

1. Sur la page [Releases](https://github.com/befa160/shiori/releases), téléchargez l'archive de votre système :

   | Système | Fichier |
   |---|---|
   | Linux (PC) | `shiori-<version>-x86_64-unknown-linux-musl.tar.gz` |
   | Linux (Raspberry Pi 4/5, ARM 64 bits) | `shiori-<version>-aarch64-unknown-linux-musl.tar.gz` |
   | Windows | `shiori-<version>-x86_64-pc-windows-msvc.zip` |
   | macOS (Apple Silicon) | `shiori-<version>-aarch64-apple-darwin.tar.gz` |

   Facultatif : comparez son empreinte avec le fichier `SHA256SUMS` (`sha256sum -c SHA256SUMS --ignore-missing` sous Linux), ou vérifiez qu'elle a bien été construite par ce dépôt avec la [CLI GitHub](https://cli.github.com/) : `gh attestation verify <archive> -R befa160/shiori` (`oci://ghcr.io/befa160/shiori:<version>` pour l'image Docker). Possible à partir de la 1.0.

2. Décompressez l'archive, puis lancez Shiori :
   - **Windows** : double-cliquez sur `shiori.exe`. Au premier lancement, Windows peut afficher « Windows a protégé votre ordinateur » : **Informations complémentaires**, puis **Exécuter quand même**. Shiori ouvre son interface dans votre navigateur ; la fenêtre noire reste ouverte tant qu'il tourne (la fermer l'arrête). Interface fermée par erreur ? Double-cliquez de nouveau : il la rouvre.
   - **Linux** : `./shiori` dans un terminal (ou un double-clic) ; l'interface s'ouvre dans le navigateur.
   - **macOS** : le programme n'est pas signé par Apple. Une seule fois, dans un terminal : `xattr -d com.apple.quarantine shiori`, puis `./shiori`.

3. Sinon, ouvrez <http://localhost:7878>.

Sans option, Shiori garde ses données dans le dossier habituel de votre système :

| Système | Base (à sauvegarder) | Cache (se reconstruit) |
|---|---|---|
| Linux | `~/.local/share/shiori/` | `~/.cache/shiori/` |
| Windows | `%APPDATA%\shiori\data\` | `%LOCALAPPDATA%\shiori\cache\` |
| macOS | `~/Library/Application Support/shiori/` | `~/Library/Caches/shiori/` |

Leur emplacement exact s'affiche au démarrage et dans **Administration → Données**. Pour les déplacer : [configuration](/installation/configuration).

## Premiers pas

![Page d'installation : jeton d'installation, nom et mot de passe du compte administrateur](/screens/fr/ecran_installation.jpg)

1. **Créez le compte administrateur.** Au premier lancement, l'interface affiche la page **Installation** : un nom de compte, un mot de passe (8 caractères au moins) et le **jeton d'installation**, affiché au démarrage dans la fenêtre de Shiori ou dans `docker compose logs shiori`. Il change à chaque lancement. Sans interface : `shiori user add <nom> --admin` (Docker : `docker compose exec shiori shiori user add <nom> --admin`) donne un mot de passe provisoire.
2. **Administration → Ajouter une bibliothèque** : un nom, le chemin complet du dossier (avec Docker : le chemin de droite, par exemple `/bibliotheques/mangas`) et son type (manga, BD ou webtoon).
3. Le **scan** démarre tout seul ; son avancement s'affiche dans l'administration. Un premier scan lit chaque fichier en entier : comptez quelques minutes pour une grande bibliothèque. Ensuite, la [surveillance des dossiers](/guide/scans#surveillance-des-dossiers) scanne seule ce qui change.
4. Vos séries apparaissent sur l'accueil. Rangez vos fichiers **un dossier par série** : voir [Ranger ses fichiers](/guide/libraries#ranger-ses-fichiers).
5. **Créez les comptes de la famille** dans **Administration → Comptes** ([comptes](/guide/accounts)).

Facultatif : **Administration → Métadonnées web** complète vos séries avec AniList, MangaDex et Wikipédia (désactivé par défaut ; seul le titre des séries est envoyé).

## Mettre à jour

La base se met à jour toute seule au lancement de la nouvelle version ; si son format change, Shiori la sauvegarde d'abord.

- **Docker** : `docker compose pull && docker compose up -d`.
- **Programme seul** : arrêtez Shiori, remplacez le programme par celui de la nouvelle archive, relancez-le.

### Ce qu'une mise à jour garantit (à partir de la 1.0)

Les numéros de version suivent `majeure.mineure.correctif`. D'une version 1.x à une autre :

- **La base est gardée** : son format évolue par des migrations sans perte, et Shiori la sauvegarde avant chacune. Revenir à une version plus ancienne n'est pas prévu : restaurez la sauvegarde `shiori-avant-…` faite avant la mise à jour.
- **Les identifiants des tomes et des pages ne changent pas** : progression, liens enregistrés, tomes téléchargés et caches restent valables. Un fichier modifié reçoit de nouveaux identifiants, comme toujours.
- **L'API `/api` ne fait que s'enrichir** : nouvelles routes, nouveaux champs dans les réponses, nouveaux codes d'erreur. Rien n'est retiré ni renommé, et un champ ne change pas de sens. Un client doit donc ignorer les champs qu'il ne connaît pas.
- **Les options de lancement et les variables d'environnement restent valables.**

Toute rupture de l'une de ces promesses attend une version **majeure** (2.0), annoncée dans les notes de version. Les préversions (`1.0.0-rc.1`) servent à l'essai : elles ne promettent rien et ne deviennent jamais `latest`.
