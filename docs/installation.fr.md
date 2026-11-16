# Installer Shiori

[English version](installation.en.md)

Shiori est un seul programme : il lit vos dossiers de mangas, BD et webtoons et les sert à une application web que vous ouvrez dans le navigateur de votre tablette, téléphone ou ordinateur. Il ne modifie jamais vos fichiers.

Deux façons de l'installer :

- **[Docker](#avec-docker)** : le plus simple sur un NAS, un serveur ou un Raspberry Pi.
- **[Le programme seul](#avec-le-programme-seul)** : Linux, Windows ou macOS, sans rien d'autre à installer.

> **Important : Shiori n'a pas encore de comptes ni de mot de passe.** Toute personne qui atteint son adresse peut lire et administrer la bibliothèque. Par défaut, il n'est joignable que depuis l'appareil où il tourne. Pour l'ouvrir à votre réseau ou à Internet, suivez [Accéder depuis d'autres appareils](#accéder-depuis-dautres-appareils).

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

L'image tourne sans droits d'administrateur et garde ses données dans deux volumes Docker : `shiori-data` (la base : progression, métadonnées, couvertures, **à sauvegarder**) et `shiori-cache` (pages redimensionnées, qui se reconstruit tout seul).

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

1. **Administration → Ajouter une bibliothèque** : un nom, le chemin complet du dossier (avec Docker : le chemin de droite, par exemple `/bibliotheques/mangas`) et son type (manga, BD ou webtoon, qui donne le sens de lecture par défaut).
2. Le scan démarre tout seul ; son avancement s'affiche dans l'administration. Un premier scan lit chaque fichier en entier : comptez quelques minutes pour une grande bibliothèque. Les suivants ne relisent que ce qui a changé.
3. Vos séries apparaissent sur l'accueil. Rangez vos fichiers **un dossier par série** : voir [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers). Les **liens symboliques** ne sont pas suivis : montez le dossier lui-même (volume Docker, lien dur, montage réseau). Si le dossier d'une bibliothèque est absent ou illisible (disque réseau non monté), le scan échoue sans rien retirer.

Facultatif : **Administration → Métadonnées web** complète vos séries (résumé, auteurs, genres, couvertures) avec AniList et MangaDex. C'est désactivé par défaut ; seul le titre des séries est envoyé.

## Accéder depuis d'autres appareils

### Sur votre réseau local

Shiori n'écoute que l'appareil où il tourne. Pour le joindre depuis la tablette, faites-le écouter sur le réseau **et** dites-lui par quel nom ou adresse vous l'ouvrez (sinon il refuse la page, par sécurité) :

- programme seul : `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (l'adresse de l'ordinateur) ;
- Docker : dans `compose.yaml`, `"7878:7878"` au lieu de `"127.0.0.1:7878:7878"`, et `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Puis ouvrez `http://192.168.1.10:7878` sur la tablette. Ne faites cela que sur un réseau de confiance (votre maison), puisqu'il n'y a pas de mot de passe.

### Depuis Internet, avec un mot de passe

Mettez un **reverse proxy** devant Shiori : il donne une adresse HTTPS (par exemple `https://shiori.mondomaine.fr`) et demande un mot de passe avant de laisser passer. Exemple avec [Caddy](https://caddyserver.com), qui obtient le certificat HTTPS tout seul :

```caddyfile
shiori.mondomaine.fr {
    basic_auth {
        # Mot de passe haché : caddy hash-password --plaintext 'votre mot de passe'
        lecteur $2a$14$...
    }
    reverse_proxy 127.0.0.1:7878
}
```

Et dites à Shiori ce nom : `--allowed-host shiori.mondomaine.fr` (ou `SHIORI_ALLOWED_HOSTS: localhost,shiori.mondomaine.fr` avec Docker). Nginx Proxy Manager, Traefik, Authelia ou Cloudflare Access conviennent aussi. Utilisez un **sous-domaine** : Shiori ne fonctionne pas dans un sous-chemin (`mondomaine.fr/shiori`).

Avec Cloudflare (compte gratuit) : la limite de 100 Mo porte sur chaque requête, et Shiori n'envoie et ne reçoit que des pages de quelques centaines de Ko, y compris pour les tomes téléchargés.

### La même adresse à la maison et dehors

À la maison, inutile de passer par Internet : faites pointer le même nom vers le reverse proxy local avec un **DNS local** (réglage de votre box, Pi-hole, AdGuard Home : `shiori.mondomaine.fr` → l'adresse locale du reverse proxy). Le reverse proxy doit avoir un certificat valable pour ce nom (Caddy ou Let's Encrypt par défi DNS). Vous gardez une seule adresse, donc une seule application installée et les mêmes tomes téléchargés, et à la maison le trafic ne sort pas.

## Installer l'application sur la tablette ou le téléphone

Shiori s'installe comme une application (PWA), en plein écran, sans la barre du navigateur :

- **Android (Chrome)** : menu ⋮ → **Installer l'application** (ou « Ajouter à l'écran d'accueil »).
- **iPad, iPhone (Safari)** : bouton Partager → **Sur l'écran d'accueil**.

L'installation, la lecture hors ligne et les tomes téléchargés demandent une adresse **HTTPS** (ou `localhost`) : les navigateurs les refusent sur `http://192.168…`. Passez par un reverse proxy comme ci-dessus.

## Mettre à jour

Sauvegardez d'abord la base (ci-dessous). La base se met à jour toute seule au lancement de la nouvelle version.

- **Docker** : `docker compose pull && docker compose up -d`.
- **Programme seul** : arrêtez Shiori, remplacez le programme par celui de la nouvelle archive, relancez-le.

## Sauvegarder

Tout ce qui compte (bibliothèques, progression, métadonnées, couvertures choisies) est dans **un seul fichier, `shiori.db`**. Le cache n'a pas besoin de sauvegarde. Arrêtez Shiori avant de copier la base, pour une copie cohérente :

- **Docker** :

  ```bash
  docker compose stop
  docker compose cp shiori:/data/shiori.db ./shiori-sauvegarde.db
  docker compose start
  ```

- **Programme seul** : arrêtez Shiori et copiez `shiori.db` depuis le dossier de données (tableau plus haut).

Pour restaurer : arrêtez Shiori, remettez le fichier à sa place, relancez.

## En cas de problème

- **« nom d'hôte non autorisé »** : vous ouvrez Shiori par un nom ou une adresse qu'il ne connaît pas. Ajoutez-le à `--allowed-host` / `SHIORI_ALLOWED_HOSTS`.
- **Docker, « unable to open database file »** : un dossier de l'hôte monté sur `/data` appartient à root. Gardez les volumes nommés du `compose.yaml` fourni, ou donnez ce dossier à l'utilisateur du conteneur (`user:` dans `compose.yaml`).
- **Une série manque ou est mal découpée** : voir [Ranger ses fichiers](fonctionnel.md#ranger-ses-fichiers) et le journal du scan (Administration → Historique et options).
- **Un tome est en erreur** : son menu → « Problèmes du scan » ; chaque code est expliqué dans [la documentation fonctionnelle](fonctionnel.md#codes-de-diagnostic).
