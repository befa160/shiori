# Backups

Everything that matters (libraries, progress, accounts, metadata, chosen covers) is in the `shiori.db` database. The cache needs no backup.

## Automatic

Shiori backs up its database **on its own, once a day**, in the `backups` folder next to it (`/data/backups` with Docker), and keeps the 7 most recent. It also makes one before each update of the database format and before each restore (`shiori-avant-…`, never deleted automatically). A backup is consistent even while Shiori runs.

**Administration → Data** shows the folder, the frequency and how many are kept, and lets you back up right away, download or delete a backup.

Options: `--backup-dir` / `SHIORI_BACKUP_DIR`, `--backup-every-hours` / `SHIORI_BACKUP_EVERY_HOURS` (24, 0 for none), `--backup-keep` / `SHIORI_BACKUP_KEEP` (7). `shiori backup` makes one on request (`docker compose exec shiori shiori backup`).

## To the cloud

Your backup tool (Backrest, restic, Duplicati…) only has to copy that folder; each file in it is complete (written under a temporary name, then renamed). Example with Backrest in Docker: mount Shiori's volume read-only in its container, then create a plan on `/userdata/shiori/backups`.

```yaml
services:
  backrest:
    volumes:
      - shiori-data:/userdata/shiori:ro
volumes:
  shiori-data:
    external: true
    name: shiori_shiori-data # real name: docker volume ls
```

::: warning
Backups contain password hashes and two-factor secrets: keep them encrypted (Backrest and restic do).
:::

## Restoring

In **Administration → Data**: the restore button of a backup in the list, or **Restore from a file…** for a backup recovered elsewhere. Shiori checks the file, brings it to its version's format, first backs up the current database, then replaces it without restarting. Everyone then signs in again, with the accounts and passwords of the backup; API tokens also go back to their state at that time. A backup from a newer version of Shiori is refused: update Shiori first.

If Shiori no longer starts: stop it, replace `shiori.db` with a backup, **delete `shiori.db-wal` and `shiori.db-shm`** if they exist, start it again.
