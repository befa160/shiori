# Installer Shiori

[English version](installation.en.md)

Shiori est un seul programme : il lit vos dossiers de mangas, BD et webtoons et les sert à une application web que vous ouvrez dans le navigateur de votre tablette, téléphone ou ordinateur. Il ne modifie jamais vos fichiers.

Deux façons de l'installer :

- **[Docker](#avec-docker)** : le plus simple sur un NAS, un serveur ou un Raspberry Pi.
- **[Le programme seul](#avec-le-programme-seul)** : Linux, Windows ou macOS, sans rien d'autre à installer.

Chaque personne a son **compte** (sa progression, ses réglages, les bibliothèques qui lui sont ouvertes). Par défaut, Shiori n'est joignable que depuis l'appareil où il tourne ; pour l'ouvrir à votre réseau ou à Internet, suivez [Accéder depuis d'autres appareils](#accéder-depuis-dautres-appareils).

## Avec Docker

Il faut Docker et Docker Compose ([installer Docker](https://docs.docker.com/get-started/get-docker/)).

1. Créez un dossier, par exemple `shiori`, et placez-y le fichier [`compose.yaml`](../compose.yaml) du dépôt.
2. Ouvrez `compose.yaml` et dites où sont vos fichiers : remplacez `/chemin/vers/mangas` par le vrai dossier, puis enlevez le `#` en début de ligne :

   ```yaml
   - /srv/mangas:/bibliotheques/mangas:ro
   ```

   La partie de droite (`/bibliotheques/mangas`) est le chemin vu par Shiori : c'est celui que vous donnerez dans l'interface. `:ro` le rend en lecture seule. Ajoutez une ligne par dossier (BD, webtoons…).
3. Lancez Shiori :

   ```bash
   docker compose up -d
   ```

4. Ouvrez <http://localhost:7878> et passez à [Premiers pas](#premiers-pas).

L'image tourne sans droits d'administrateur et garde ses données dans deux volumes Docker : `shiori-data` (la base : comptes, progression, métadonnées, couvertures, et ses [sauvegardes](#sauvegarder)) et `shiori-cache` (pages redimensionnées, qui se reconstruit tout seul).

## Avec le programme seul

1. Sur la page [Releases](https://github.com/befa160/shiori/releases), téléchargez l'archive de votre système :

   | Système | Fichier |
   |---|---|
   | Linux (PC) | `shiori-<version>-x86_64-unknown-linux-musl.tar.gz` |
   | Linux (Raspberry Pi 4/5, ARM 64 bits) | `shiori-<version>-aarch64-unknown-linux-musl.tar.gz` |
   | Windows | `shiori-<version>-x86_64-pc-windows-msvc.zip` |
   | macOS (Apple Silicon) | `shiori-<version>-aarch64-apple-darwin.tar.gz` |

   Facultatif : comparez son empreinte avec le fichier `SHA256SUMS` (`sha256sum -c SHA256SUMS --ignore-missing` sous Linux).
2. Décompressez l'archive, puis lancez Shiori :
   - **Windows** : double-cliquez sur `shiori.exe`. Au premier lancement, Windows peut afficher « Windows a protégé votre ordinateur » : cliquez sur **Informations complémentaires**, puis **Exécuter quand même**. Shiori ouvre son interface dans votre navigateur. Une fenêtre noire reste ouverte tant que Shiori tourne (elle indique l'adresse et l'emplacement des données) ; la fermer l'arrête. Interface fermée par erreur ? Double-cliquez de nouveau sur `shiori.exe` : il la rouvre.
   - **Linux** : dans un terminal, `./shiori` (ou un double-clic depuis le gestionnaire de fichiers) ; l'interface s'ouvre dans le navigateur.
   - **macOS** : le programme n'est pas signé par Apple. Dans un terminal, une seule fois : `xattr -d com.apple.quarantine shiori`, puis `./shiori` ; l'interface s'ouvre dans le navigateur.
3. Sinon, ouvrez <http://localhost:7878>, puis passez à [Premiers pas](#premiers-pas).

Sans option, Shiori garde ses données dans le dossier habituel de votre système :

| Système | Base (à sauvegarder) | Cache (se reconstruit) |
|---|---|---|
| Linux | `~/.local/share/shiori/` | `~/.cache/shiori/` |
| Windows | `%APPDATA%\shiori\data\` | `%LOCALAPPDATA%\shiori\cache\` |
| macOS | `~/Library/Application Support/shiori/` | `~/Library/Caches/shiori/` |

Leur emplacement exact s'affiche dans **Administration → Données** et au démarrage. Pour les déplacer : variables `SHIORI_DATA_DIR` et `SHIORI_CACHE_DIR`, ou options `--db` et `--cache-dir`. Toutes les options : `shiori serve --help`.

## Premiers pas

1. **Créez le compte administrateur.** Au premier lancement, l'interface affiche la page **Installation** : choisissez un nom de compte et un mot de passe (8 caractères au moins), et recopiez le **jeton d'installation**. Ce jeton s'affiche au démarrage dans la fenêtre de Shiori, ou dans le journal Docker (`docker compose logs shiori`) ; il change à chaque lancement et empêche le premier venu de votre réseau de prendre l'instance. Le compte créé reprend la progression et les réglages déjà enregistrés par une version précédente. Sans interface : `shiori user add <nom> --admin` (avec Docker : `docker compose exec shiori shiori user add <nom> --admin`) affiche un mot de passe provisoire, à changer à la première connexion.
2. **Administration → Ajouter une bibliothèque** : un nom, le chemin complet du dossier (avec Docker : le chemin de droite, par exemple `/bibliotheques/mangas`) et son type (manga, BD ou webtoon, qui donne le sens de lecture par défaut).
3. Le scan démarre tout seul ; son avancement s'affiche dans l'administration. Un premier scan lit chaque fichier en entier : comptez quelques minutes pour une grande bibliothèque. Les suivants ne relisent que ce qui a changé.
4. Vos séries apparaissent sur l'accueil. Rangez vos fichiers **un dossier par série** : voir [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers). Les **liens symboliques** ne sont pas suivis : montez le dossier lui-même (volume Docker, lien dur, montage réseau). Si le dossier d'une bibliothèque est absent ou illisible (disque réseau non monté), le scan échoue sans rien retirer.

5. **Créez les comptes de la famille** dans **Administration → Comptes** : un nom, le rôle (lecteur ou administrateur) et les bibliothèques ouvertes. Shiori donne un mot de passe provisoire, montré une seule fois, que la personne remplace à sa première connexion. Un lecteur lit, garde sa progression et ses réglages, et ne voit ni l'administration ni les autres comptes. Une bibliothèque créée plus tard n'est visible que de l'administrateur : ouvrez-la aux comptes voulus dans **Administration → Comptes**.

Facultatif : **Administration → Métadonnées web** complète vos séries (résumé, auteurs, genres, couvertures) avec AniList et MangaDex. C'est désactivé par défaut ; seul le titre des séries est envoyé.

## Accéder depuis d'autres appareils

### Sur votre réseau local

Shiori n'écoute que l'appareil où il tourne. Pour le joindre depuis la tablette, faites-le écouter sur le réseau **et** dites-lui par quel nom ou adresse vous l'ouvrez (sinon il refuse la page, par sécurité) :

- programme seul : `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (l'adresse de l'ordinateur) ;
- Docker : dans `compose.yaml`, `"7878:7878"` au lieu de `"127.0.0.1:7878:7878"`, et `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Puis ouvrez `http://192.168.1.10:7878` sur la tablette et connectez-vous. En `http://`, le mot de passe circule en clair sur le réseau : réservez cela à un réseau de confiance (votre maison).

### Depuis Internet

Mettez un **reverse proxy** devant Shiori : il donne une adresse HTTPS (par exemple `https://shiori.mondomaine.fr`), pour que mots de passe et sessions ne circulent jamais en clair. Exemple avec [Caddy](https://caddyserver.com), qui obtient le certificat HTTPS tout seul :

```caddyfile
shiori.mondomaine.fr {
    reverse_proxy 127.0.0.1:7878
}
```

Et dites à Shiori ce nom : `--allowed-host shiori.mondomaine.fr` (ou `SHIORI_ALLOWED_HOSTS: localhost,shiori.mondomaine.fr` avec Docker). Nginx Proxy Manager, Traefik ou Cloudflare Tunnel conviennent aussi. Utilisez un **sous-domaine** : Shiori ne fonctionne pas dans un sous-chemin (`mondomaine.fr/shiori`).

Ouvert sur Internet, Shiori se protège seul : connexion obligatoire, pause qui s'allonge après plusieurs mots de passe faux (par compte et par adresse), cookie de session réservé au HTTPS derrière le proxy (`X-Forwarded-Proto`, `X-Forwarded-For`, que les reverse proxies ajoutent d'eux-mêmes). Activez la **double authentification** sur les comptes administrateurs (**Compte → Double authentification**, avec une application comme Aegis ou Google Authenticator). Une protection en plus devant Shiori (mot de passe du proxy, Authelia, Cloudflare Access) reste possible ; l'application installée la redemande quand il le faut.

Avec Cloudflare (compte gratuit) : la limite de 100 Mo porte sur chaque requête, et Shiori n'envoie et ne reçoit que des pages de quelques centaines de Ko, y compris pour les tomes téléchargés.

### La même adresse à la maison et dehors

À la maison, inutile de passer par Internet : faites pointer le même nom vers le reverse proxy local avec un **DNS local** (réglage de votre box, Pi-hole, AdGuard Home : `shiori.mondomaine.fr` → l'adresse locale du reverse proxy). Le reverse proxy doit avoir un certificat valable pour ce nom (Caddy ou Let's Encrypt par défi DNS). Vous gardez une seule adresse, donc une seule application installée et les mêmes tomes téléchargés, et à la maison le trafic ne sort pas.

## Installer l'application sur la tablette ou le téléphone

Shiori s'installe comme une application (PWA), en plein écran, sans la barre du navigateur :

- **Android (Chrome)** : menu ⋮ → **Installer l'application** (ou « Ajouter à l'écran d'accueil »).
- **iPad, iPhone (Safari)** : bouton Partager → **Sur l'écran d'accueil**.

L'installation, la lecture hors ligne et les tomes téléchargés demandent une adresse **HTTPS** (ou `localhost`) : les navigateurs les refusent sur `http://192.168…`. Passez par un reverse proxy comme ci-dessus.

## Mettre à jour

La base se met à jour toute seule au lancement de la nouvelle version ; si son format change, Shiori la sauvegarde d'abord (voir ci-dessous).

- **Docker** : `docker compose pull && docker compose up -d`.
- **Programme seul** : arrêtez Shiori, remplacez le programme par celui de la nouvelle archive, relancez-le.

## Sauvegarder

Tout ce qui compte (bibliothèques, progression, comptes, métadonnées, couvertures choisies) est dans la base `shiori.db`. Le cache n'a pas besoin de sauvegarde.

Shiori sauvegarde sa base **tout seul, une fois par jour**, dans le dossier `backups` à côté d'elle (`/data/backups` avec Docker), et garde les 7 plus récentes. Il en fait aussi une avant chaque mise à jour du schéma de la base et avant chaque restauration (`shiori-avant-…`, jamais effacées d'office). Le dossier, la fréquence et le nombre gardé sont affichés dans **Administration → Données**, où l'on peut aussi sauvegarder tout de suite, télécharger ou supprimer une sauvegarde.

Options : `--backup-dir` / `SHIORI_BACKUP_DIR` (dossier), `--backup-every-hours` / `SHIORI_BACKUP_EVERY_HOURS` (24 par défaut, 0 pour aucune), `--backup-keep` / `SHIORI_BACKUP_KEEP` (7). `shiori backup` en fait une à la demande, Shiori allumé ou non (`docker compose exec shiori shiori backup`).

**Vers le cloud** : votre outil de sauvegarde (Backrest, restic, Duplicati…) n'a qu'à copier ce dossier ; chaque fichier y est complet (écrit sous un nom temporaire puis renommé). Exemple avec Backrest dans Docker : montez le volume de Shiori en lecture seule dans son conteneur, puis créez un plan sur `/userdata/shiori/backups`.

```yaml
services:
  backrest:
    volumes:
      - shiori-data:/userdata/shiori:ro
volumes:
  shiori-data:
    external: true
    name: shiori_shiori-data # nom réel : docker volume ls
```

Les sauvegardes contiennent les empreintes des mots de passe et les secrets de double authentification : gardez-les chiffrées (Backrest et restic le font).

### Restaurer

Dans **Administration → Données** : le bouton de restauration d'une sauvegarde de la liste, ou **Restaurer depuis un fichier…** pour une sauvegarde récupérée ailleurs (depuis Backrest, par exemple). Shiori vérifie le fichier, le met au format de sa version, sauvegarde d'abord la base actuelle, puis la remplace sans redémarrer. Tout le monde doit ensuite se reconnecter, avec les comptes et mots de passe de la sauvegarde ; les jetons d'API reviennent eux aussi à leur état d'alors, supprimez de nouveau ceux révoqués depuis. Une sauvegarde d'une version plus récente de Shiori est refusée : mettez Shiori à jour d'abord.

Si Shiori ne démarre plus : arrêtez-le, remplacez `shiori.db` par une sauvegarde, **supprimez `shiori.db-wal` et `shiori.db-shm`** s'ils existent, relancez.

## En cas de problème

- **Mot de passe oublié** : l'administrateur en donne un nouveau dans **Administration → Comptes** (mot de passe provisoire). Pour l'administrateur lui-même : `shiori user password <nom>` (avec Docker : `docker compose exec shiori shiori user password <nom>`), qui ferme aussi ses sessions.
- **Téléphone de la double authentification perdu** : connectez-vous avec un **code de secours** (donnés à l'activation, chacun sert une fois). Sinon, un administrateur la retire dans **Administration → Comptes**, ou `shiori user totp-reset <nom>`.
- **« Trop d’essais »** : après plusieurs mots de passe faux, Shiori fait patienter avant le prochain essai, de plus en plus longtemps (15 minutes au plus).
- **Jeton d'installation introuvable** : il n'est demandé que tant qu'aucun compte n'existe ; il s'affiche à chaque démarrage dans la fenêtre de Shiori ou dans `docker compose logs shiori`.

- **« nom d'hôte non autorisé »** : vous ouvrez Shiori par un nom ou une adresse qu'il ne connaît pas. Ajoutez-le à `--allowed-host` / `SHIORI_ALLOWED_HOSTS`.
- **Docker, « unable to open database file »** : un dossier de l'hôte monté sur `/data` appartient à root. Gardez les volumes nommés du `compose.yaml` fourni, ou donnez ce dossier à l'utilisateur du conteneur (`user:` dans `compose.yaml`).
- **Une série manque ou est mal découpée** : voir [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers) et le journal du scan (Administration → Historique et options).
- **Un tome est en erreur** : son menu → « Problèmes du scan » ; chaque code est expliqué dans [la documentation fonctionnelle](fonctionnel.md#codes-de-diagnostic).
