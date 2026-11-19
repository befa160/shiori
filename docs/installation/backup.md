# Sauvegardes

Tout ce qui compte (bibliothèques, progression, comptes, métadonnées, couvertures choisies) est dans la base `shiori.db`. Le cache n'a pas besoin de sauvegarde.

## Automatiques

Shiori sauvegarde sa base **tout seul, une fois par jour**, dans le dossier `backups` à côté d'elle (`/data/backups` avec Docker), et garde les 7 plus récentes. Il en fait aussi une avant chaque mise à jour du format de la base et avant chaque restauration (`shiori-avant-…`, jamais effacées d'office). Une sauvegarde est cohérente même Shiori allumé.

**Administration → Données** montre le dossier, la fréquence et le nombre gardé, et permet de sauvegarder tout de suite, de télécharger ou de supprimer une sauvegarde.

Options : `--backup-dir` / `SHIORI_BACKUP_DIR`, `--backup-every-hours` / `SHIORI_BACKUP_EVERY_HOURS` (24, 0 pour aucune), `--backup-keep` / `SHIORI_BACKUP_KEEP` (7). `shiori backup` en fait une à la demande (`docker compose exec shiori shiori backup`).

## Vers le cloud

Votre outil de sauvegarde (Backrest, restic, Duplicati…) n'a qu'à copier ce dossier ; chaque fichier y est complet (écrit sous un nom temporaire puis renommé). Exemple avec Backrest dans Docker : montez le volume de Shiori en lecture seule dans son conteneur, puis créez un plan sur `/userdata/shiori/backups`.

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

::: warning
Les sauvegardes contiennent les empreintes des mots de passe et les secrets de double authentification : gardez-les chiffrées (Backrest et restic le font).
:::

## Restaurer

Dans **Administration → Données** : le bouton de restauration d'une sauvegarde de la liste, ou **Restaurer depuis un fichier…** pour une sauvegarde récupérée ailleurs. Shiori vérifie le fichier, le met au format de sa version, sauvegarde d'abord la base actuelle, puis la remplace sans redémarrer. Tout le monde doit ensuite se reconnecter, avec les comptes et mots de passe de la sauvegarde ; les jetons d'API reviennent eux aussi à leur état d'alors. Une sauvegarde d'une version plus récente de Shiori est refusée : mettez Shiori à jour d'abord.

Si Shiori ne démarre plus : arrêtez-le, remplacez `shiori.db` par une sauvegarde, **supprimez `shiori.db-wal` et `shiori.db-shm`** s'ils existent, relancez.
