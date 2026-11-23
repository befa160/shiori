# Configuration

Shiori is configured with command-line options or environment variables; interface settings (language, theme, watching, metadata…) live in the database. Started without arguments (double-click), it is the same as `shiori serve --open`.

## `shiori serve`

| Option | Variable | Default | Role |
|---|---|---|---|
| `--listen <address:port>` | | `127.0.0.1:7878` | listen address; `0.0.0.0:7878` opens it to the network (sign-in required) |
| `--allowed-host <name>` | `SHIORI_ALLOWED_HOSTS` | | name or address you open Shiori with, repeatable (variable: comma-separated); DNS rebinding protection |
| `--trusted-proxy <IP>` | `SHIORI_TRUSTED_PROXIES` | any local address | address of the reverse proxy whose `X-Forwarded-For` header is trusted, repeatable; give it when Shiori can be reached directly from the local network |
| `--db <file>` | `SHIORI_DATA_DIR` (folder) | OS data folder | SQLite database |
| `--cache-dir <folder>` | `SHIORI_CACHE_DIR` | OS cache folder | resized pages, extracted solid RARs |
| `--cache-max-mib <MiB>` | | `2048` | maximum cache size; beyond it, what was read least recently goes |
| `--jobs <N>` | | number of cores | files scanned in parallel |
| `--scan-log-days <days>` | | `30` | how long scan passes and their logs are kept |
| `--backup-dir <folder>` | `SHIORI_BACKUP_DIR` | `backups` next to the database | backup folder |
| `--backup-every-hours <h>` | `SHIORI_BACKUP_EVERY_HOURS` | `24` | one backup every N hours, 0 for none |
| `--backup-keep <N>` | `SHIORI_BACKUP_KEEP` | `7` | periodic backups kept |
| `--open` | | | opens the interface in the browser once ready |

General options: `-v` / `-vv` (more logs), `--quiet`. Detailed logs: `RUST_LOG=shiori=debug` (including the time of each page render, "page rendue … ms", to judge a small machine). Full help: `shiori serve --help`.

In the Docker image: database in `/data`, cache in `/cache`, listening on `0.0.0.0:7878`, `SHIORI_ALLOWED_HOSTS=localhost`.

## Other commands

| Command | Role |
|---|---|
| `shiori scan <folders>…` | integrity scan without a server ([options](/en/guide/scans#on-the-command-line)) |
| `shiori user add <name> [--admin]` | creates an account with a temporary password, shown once |
| `shiori user password <name>` | new temporary password, closes the account's sessions |
| `shiori user totp-reset <name>` | removes an account's two-factor authentication |
| `shiori backup` | a backup right away, Shiori running or not |
| `shiori health [--addr 127.0.0.1:7878]` | asks the running Shiori for `/api/health`: exit code 0 if it answers, 1 otherwise (`SHIORI_HEALTH_ADDR`) |

With Docker, prefix with `docker compose exec shiori` (for example `docker compose exec shiori shiori backup`).

## Settings in the interface

| Setting | Where | Default |
|---|---|---|
| Folder watching | Administration → Libraries, per library | on |
| Periodic full scan | Administration → Libraries | once a day |
| Web metadata | Administration → Web metadata | off |
| Language, theme, page turn | Administration (default) and Account (per person) | browser language, dark, reading direction |
