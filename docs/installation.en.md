# Installing Shiori

[Version française](installation.fr.md)

Shiori is a single program: it reads your manga, comic and webtoon folders and serves them to a web app you open in the browser of your tablet, phone or computer. It never modifies your files.

Two ways to install it:

- **[Docker](#with-docker)**: the simplest on a NAS, a server or a Raspberry Pi.
- **[The standalone program](#with-the-standalone-program)**: Linux, Windows or macOS, nothing else to install.

Each person has their own **account** (reading progress, settings, the libraries opened to them). By default, Shiori only answers the device it runs on; to open it to your network or the Internet, follow [Access from other devices](#access-from-other-devices).

## With Docker

You need Docker and Docker Compose ([install Docker](https://docs.docker.com/get-started/get-docker/)).

1. Create a folder, for example `shiori`, and put the repository's [`compose.yaml`](../compose.yaml) in it.
2. Open `compose.yaml` and tell it where your files are: replace `/chemin/vers/mangas` with the real folder, then remove the `#` at the start of the line:

   ```yaml
   - /srv/mangas:/bibliotheques/mangas:ro
   ```

   The right-hand part (`/bibliotheques/mangas`) is the path Shiori sees: that is the one you will type in the app. `:ro` makes it read-only. Add one line per folder (comics, webtoons…).
3. Start Shiori:

   ```bash
   docker compose up -d
   ```

4. Open <http://localhost:7878> and go to [First steps](#first-steps).

The image runs without administrator rights and keeps its data in two Docker volumes: `shiori-data` (the database: accounts, reading progress, metadata, covers, and its [backups](#backing-up)) and `shiori-cache` (resized pages, rebuilt on its own).

## With the standalone program

1. On the [Releases](https://github.com/befa160/shiori/releases) page, download the archive for your system:

   | System | File |
   |---|---|
   | Linux (PC) | `shiori-<version>-x86_64-unknown-linux-musl.tar.gz` |
   | Linux (Raspberry Pi 4/5, 64-bit ARM) | `shiori-<version>-aarch64-unknown-linux-musl.tar.gz` |
   | Windows | `shiori-<version>-x86_64-pc-windows-msvc.zip` |
   | macOS (Apple Silicon) | `shiori-<version>-aarch64-apple-darwin.tar.gz` |

   Optional: check its fingerprint against the `SHA256SUMS` file (`sha256sum -c SHA256SUMS --ignore-missing` on Linux).
2. Unpack the archive, then start Shiori:
   - **Windows**: double-click `shiori.exe`. On first launch Windows may show "Windows protected your PC": click **More info**, then **Run anyway**. Shiori opens its interface in your browser. A black window stays open while Shiori runs (it shows the address and where the data is); closing it stops Shiori. Closed the interface by mistake? Double-click `shiori.exe` again: it reopens it.
   - **Linux**: in a terminal, `./shiori` (or double-click it in the file manager); the interface opens in the browser.
   - **macOS**: the program is not signed by Apple. In a terminal, once: `xattr -d com.apple.quarantine shiori`, then `./shiori`; the interface opens in the browser.
3. Otherwise, open <http://localhost:7878>, then go to [First steps](#first-steps).

Without options, Shiori keeps its data in your system's usual folder:

| System | Database (back it up) | Cache (rebuilt on its own) |
|---|---|---|
| Linux | `~/.local/share/shiori/` | `~/.cache/shiori/` |
| Windows | `%APPDATA%\shiori\data\` | `%LOCALAPPDATA%\shiori\cache\` |
| macOS | `~/Library/Application Support/shiori/` | `~/Library/Caches/shiori/` |

Their exact location is shown in **Administration → Data** and at startup. To move them: the `SHIORI_DATA_DIR` and `SHIORI_CACHE_DIR` variables, or the `--db` and `--cache-dir` options. All options: `shiori serve --help`.

## First steps

1. **Create the administrator account.** On first launch, the interface shows the **Setup** page: choose an account name and a password (at least 8 characters), and copy the **setup token**. The token is printed at startup in Shiori's window, or in the Docker log (`docker compose logs shiori`); it changes at every launch and stops the first comer on your network from taking over the instance. The new account takes over the reading progress and settings saved by an earlier version. Without the interface: `shiori user add <name> --admin` (with Docker: `docker compose exec shiori shiori user add <name> --admin`) prints a temporary password, to change at first sign-in.
2. **Administration → Add a library**: a name, the full path of the folder (with Docker: the right-hand path, for example `/bibliotheques/mangas`) and its type (manga, comic or webtoon, which sets the default reading direction).
3. The scan starts on its own; its progress shows in the administration page. A first scan reads every file in full: allow a few minutes for a large library. Later scans only read what changed.
4. Your series appear on the home page. Store your files **one folder per series**: see [Organising your files](fonctionnel.md#ranger-ses-fichiers) (French). **Symbolic links** are not followed: mount the folder itself (Docker volume, hard link, network mount). If a library folder is missing or unreadable (network drive not mounted), the scan fails and removes nothing.

5. **Create your family's accounts** in **Administration → Accounts**: a name, the role (reader or administrator) and the opened libraries. Shiori gives a temporary password, shown once, which the person replaces at first sign-in. A reader reads, keeps their own progress and settings, and sees neither the administration nor other accounts. A library created later is only visible to the administrator: open it to the right accounts in **Administration → Accounts**.

Optional: **Administration → Web metadata** completes your series (summary, authors, genres, covers) from AniList and MangaDex. It is off by default; only series titles are sent.

## Access from other devices

### On your local network

Shiori only listens to the device it runs on. To reach it from the tablet, make it listen on the network **and** tell it which name or address you use (otherwise it refuses the page, for safety):

- standalone program: `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (the computer's address);
- Docker: in `compose.yaml`, `"7878:7878"` instead of `"127.0.0.1:7878:7878"`, and `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Then open `http://192.168.1.10:7878` on the tablet and sign in. Over `http://`, the password travels in clear on the network: keep this to a trusted network (your home).

### From the Internet

Put a **reverse proxy** in front of Shiori: it provides an HTTPS address (for example `https://shiori.example.com`), so passwords and sessions never travel in clear. Example with [Caddy](https://caddyserver.com), which gets the HTTPS certificate on its own:

```caddyfile
shiori.example.com {
    reverse_proxy 127.0.0.1:7878
}
```

And tell Shiori that name: `--allowed-host shiori.example.com` (or `SHIORI_ALLOWED_HOSTS: localhost,shiori.example.com` with Docker). Nginx Proxy Manager, Traefik or Cloudflare Tunnel work too. Use a **subdomain**: Shiori does not work under a sub-path (`example.com/shiori`).

Open to the Internet, Shiori protects itself: sign-in required, a pause that grows after several wrong passwords (per account and per address), session cookie restricted to HTTPS behind the proxy (`X-Forwarded-Proto`, `X-Forwarded-For`, which reverse proxies add on their own). Turn on **two-factor authentication** for administrator accounts (**Account → Two-factor authentication**, with an app such as Aegis or Google Authenticator). An extra protection in front of Shiori (proxy password, Authelia, Cloudflare Access) is still possible; the installed app asks for it again when needed.

With Cloudflare (free plan): the 100 MB limit applies to each request, and Shiori only sends and receives pages of a few hundred KB, downloaded volumes included.

### The same address at home and away

At home there is no need to go through the Internet: point the same name at the local reverse proxy with **local DNS** (your router's setting, Pi-hole, AdGuard Home: `shiori.example.com` → the reverse proxy's local address). The reverse proxy needs a certificate valid for that name (Caddy, or Let's Encrypt with a DNS challenge). You keep a single address, so a single installed app and the same downloaded volumes, and home traffic stays at home.

## Installing the app on a tablet or phone

Shiori installs as an app (PWA), full screen, without the browser bar:

- **Android (Chrome)**: menu ⋮ → **Install app** (or "Add to home screen").
- **iPad, iPhone (Safari)**: Share button → **Add to Home Screen**.

Installing, offline reading and downloaded volumes need an **HTTPS** address (or `localhost`): browsers refuse them on `http://192.168…`. Use a reverse proxy as above.

## Updating

The database updates itself when the new version starts; if its format changes, Shiori backs it up first (see below).

- **Docker**: `docker compose pull && docker compose up -d`.
- **Standalone program**: stop Shiori, replace the program with the one from the new archive, start it again.

## Backing up

Everything that matters (libraries, reading progress, accounts, metadata, chosen covers) is in the `shiori.db` database. The cache needs no backup.

Shiori backs up its database **by itself, once a day**, in the `backups` folder next to it (`/data/backups` with Docker), and keeps the 7 most recent. It also makes one before each database schema update and before each restore (`shiori-avant-…`, never deleted automatically). The folder, frequency and number kept are shown in **Administration → Data**, where you can also back up right away, download or delete a backup.

Options: `--backup-dir` / `SHIORI_BACKUP_DIR` (folder), `--backup-every-hours` / `SHIORI_BACKUP_EVERY_HOURS` (24 by default, 0 for none), `--backup-keep` / `SHIORI_BACKUP_KEEP` (7). `shiori backup` makes one on demand, whether Shiori is running or not (`docker compose exec shiori shiori backup`).

**To the cloud**: your backup tool (Backrest, restic, Duplicati…) only has to copy that folder; every file in it is complete (written under a temporary name, then renamed). Example with Backrest in Docker: mount Shiori's volume read-only in its container, then create a plan on `/userdata/shiori/backups`.

```yaml
services:
  backrest:
    volumes:
      - shiori-data:/userdata/shiori:ro
volumes:
  shiori-data:
    external: true
    name: shiori_shiori-data # actual name: docker volume ls
```

Backups contain password hashes and two-factor secrets: keep them encrypted (Backrest and restic do).

### Restoring

In **Administration → Data**: the restore button of a backup in the list, or **Restore from a file…** for a backup fetched elsewhere (from Backrest, for example). Shiori checks the file, brings it to its version's format, backs up the current database first, then replaces it without restarting. Everyone then has to sign in again, with the accounts and passwords of the backup; API tokens also return to their state at that time, so revoke again those revoked since. A backup from a newer Shiori version is refused: update Shiori first.

If Shiori no longer starts: stop it, replace `shiori.db` with a backup, **delete `shiori.db-wal` and `shiori.db-shm`** if they exist, start it again.

## Troubleshooting

- **Forgotten password**: the administrator gives a new one in **Administration → Accounts** (temporary password). For the administrator themselves: `shiori user password <name>` (with Docker: `docker compose exec shiori shiori user password <name>`), which also closes their sessions.
- **Lost two-factor phone**: sign in with a **recovery code** (given when turning it on, each one works once). Otherwise, an administrator removes it in **Administration → Accounts**, or `shiori user totp-reset <name>`.
- **"Too many attempts"**: after several wrong passwords, Shiori makes you wait before the next try, longer each time (15 minutes at most).
- **Setup token not found**: it is only asked while no account exists; it is printed at every start in Shiori's window or in `docker compose logs shiori`.

- **"nom d'hôte non autorisé" (host name not allowed)**: you open Shiori by a name or address it does not know. Add it to `--allowed-host` / `SHIORI_ALLOWED_HOSTS`.
- **Docker, "unable to open database file"**: a host folder mounted on `/data` belongs to root. Keep the named volumes of the provided `compose.yaml`, or give that folder to the container's user (`user:` in `compose.yaml`).
- **A series is missing or split wrongly**: see [Organising your files](fonctionnel.md#ranger-ses-fichiers) and the scan log (Administration → History and options).
- **A volume is in error**: its menu → "Scan issues"; each code is explained in [the functional documentation](fonctionnel.md#codes-de-diagnostic) (French).
