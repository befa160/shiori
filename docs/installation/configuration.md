# Configuration

Shiori se règle par options de la ligne de commande ou variables d'environnement ; les réglages de l'interface (langue, thème, surveillance, métadonnées…) sont dans la base. Lancé sans argument (double-clic), il équivaut à `shiori serve --open`.

## `shiori serve`

| Option | Variable | Défaut | Rôle |
|---|---|---|---|
| `--listen <adresse:port>` | | `127.0.0.1:7878` | adresse d'écoute ; `0.0.0.0:7878` ouvre au réseau (connexion exigée) |
| `--allowed-host <nom>` | `SHIORI_ALLOWED_HOSTS` | | nom ou adresse par lequel vous ouvrez Shiori, répétable (variable : séparés par des virgules) ; parade au DNS rebinding |
| `--trusted-proxy <IP>` | `SHIORI_TRUSTED_PROXIES` | toute adresse locale | adresse du reverse proxy dont l'en-tête `X-Forwarded-For` est cru, répétable ; à donner quand Shiori est joignable directement depuis le réseau local |
| `--db <fichier>` | `SHIORI_DATA_DIR` (dossier) | dossier de données de l'OS | base SQLite |
| `--cache-dir <dossier>` | `SHIORI_CACHE_DIR` | dossier de cache de l'OS | pages redimensionnées, RAR solides extraits |
| `--cache-max-mib <Mio>` | | `2048` | taille maximale du cache ; au-delà, ce qui a été le moins récemment lu part |
| `--jobs <N>` | | nombre de cœurs | fichiers scannés en parallèle |
| `--scan-log-days <jours>` | | `30` | durée de conservation des passes de scan et de leurs journaux |
| `--backup-dir <dossier>` | `SHIORI_BACKUP_DIR` | `backups` à côté de la base | dossier des sauvegardes |
| `--backup-every-hours <h>` | `SHIORI_BACKUP_EVERY_HOURS` | `24` | une sauvegarde toutes les N heures, 0 pour aucune |
| `--backup-keep <N>` | `SHIORI_BACKUP_KEEP` | `7` | sauvegardes périodiques gardées |
| `--open` | | | ouvre l'interface dans le navigateur une fois prêt |

Options générales : `-v` / `-vv` (plus de journaux), `--quiet`. Journaux détaillés : `RUST_LOG=shiori=debug` (dont la durée de chaque rendu de page, « page rendue … ms », pour juger une petite machine). Aide complète : `shiori serve --help`.

Dans l'image Docker : base dans `/data`, cache dans `/cache`, écoute sur `0.0.0.0:7878`, `SHIORI_ALLOWED_HOSTS=localhost`.

## Autres commandes

| Commande | Rôle |
|---|---|
| `shiori scan <dossiers>…` | scan d'intégrité sans serveur ([options](/guide/scans#en-ligne-de-commande)) |
| `shiori user add <nom> [--admin]` | crée un compte avec un mot de passe provisoire, affiché une fois |
| `shiori user password <nom>` | nouveau mot de passe provisoire, ferme les sessions du compte |
| `shiori user totp-reset <nom>` | retire la double authentification d'un compte |
| `shiori backup` | une sauvegarde tout de suite, Shiori allumé ou non |
| `shiori health [--addr 127.0.0.1:7878]` | demande `/api/health` au Shiori lancé : code 0 s'il répond, 1 sinon (`SHIORI_HEALTH_ADDR`) |

Avec Docker, préfixez par `docker compose exec shiori` (par exemple `docker compose exec shiori shiori backup`).

## Réglages dans l'interface

| Réglage | Où | Défaut |
|---|---|---|
| Surveillance des dossiers | Administration → Bibliothèques, par bibliothèque | active |
| Scan complet périodique | Administration → Bibliothèques | une fois par jour |
| Métadonnées web | Administration → Métadonnées web | désactivées |
| Langue, thème, sens des pages | Administration (défaut) et Compte (par personne) | langue du navigateur, sombre, sens de lecture |
