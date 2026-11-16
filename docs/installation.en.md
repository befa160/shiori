# Installing Shiori

[Version française](installation.fr.md)

Shiori is a single program: it reads your manga, comic and webtoon folders and serves them to a web app you open in the browser of your tablet, phone or computer. It never modifies your files.

Two ways to install it:

- **[Docker](#with-docker)**: the simplest on a NAS, a server or a Raspberry Pi.
- **[The standalone program](#with-the-standalone-program)**: Linux, Windows or macOS, nothing else to install.

> **Important: Shiori has no accounts or passwords yet.** Anyone who can reach its address can read and administer the library. By default it only answers the device it runs on. To open it to your network or the Internet, follow [Access from other devices](#access-from-other-devices).

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

The image runs without administrator rights and keeps its data in two Docker volumes: `shiori-data` (the database: reading progress, metadata, covers, **back it up**) and `shiori-cache` (resized pages, rebuilt on its own).

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
   - **Windows**: double-click `shiori.exe`. On first launch Windows may show "Windows protected your PC": click **More info**, then **Run anyway**. A window stays open while Shiori runs; closing it stops Shiori.
   - **Linux**: in a terminal, `./shiori`.
   - **macOS**: the program is not signed by Apple. In a terminal, once: `xattr -d com.apple.quarantine shiori`, then `./shiori`.
3. Open <http://localhost:7878> and go to [First steps](#first-steps).

Without options, Shiori keeps its data in your system's usual folder:

| System | Database (back it up) | Cache (rebuilt on its own) |
|---|---|---|
| Linux | `~/.local/share/shiori/` | `~/.cache/shiori/` |
| Windows | `%APPDATA%\shiori\data\` | `%LOCALAPPDATA%\shiori\cache\` |
| macOS | `~/Library/Application Support/shiori/` | `~/Library/Caches/shiori/` |

To move them: the `SHIORI_DATA_DIR` and `SHIORI_CACHE_DIR` variables, or the `--db` and `--cache-dir` options. All options: `shiori serve --help`.

## First steps

1. **Administration → Add a library**: a name, the full path of the folder (with Docker: the right-hand path, for example `/bibliotheques/mangas`) and its type (manga, comic or webtoon, which sets the default reading direction).
2. The scan starts on its own; its progress shows in the administration page. A first scan reads every file in full: allow a few minutes for a large library. Later scans only read what changed.
3. Your series appear on the home page. Store your files **one folder per series**: see [Organising your files](fonctionnel.md#ranger-ses-fichiers) (French).

Optional: **Administration → Web metadata** completes your series (summary, authors, genres, covers) from AniList and MangaDex. It is off by default; only series titles are sent.

## Access from other devices

### On your local network

Shiori only listens to the device it runs on. To reach it from the tablet, make it listen on the network **and** tell it which name or address you use (otherwise it refuses the page, for safety):

- standalone program: `shiori serve --listen 0.0.0.0:7878 --allowed-host 192.168.1.10` (the computer's address);
- Docker: in `compose.yaml`, `"7878:7878"` instead of `"127.0.0.1:7878:7878"`, and `SHIORI_ALLOWED_HOSTS: localhost,192.168.1.10`.

Then open `http://192.168.1.10:7878` on the tablet. Only do this on a trusted network (your home), since there is no password.

### From the Internet, with a password

Put a **reverse proxy** in front of Shiori: it provides an HTTPS address (for example `https://shiori.example.com`) and asks for a password before letting anyone through. Example with [Caddy](https://caddyserver.com), which gets the HTTPS certificate on its own:

```caddyfile
shiori.example.com {
    basic_auth {
        # Hashed password: caddy hash-password --plaintext 'your password'
        reader $2a$14$...
    }
    reverse_proxy 127.0.0.1:7878
}
```

And tell Shiori that name: `--allowed-host shiori.example.com` (or `SHIORI_ALLOWED_HOSTS: localhost,shiori.example.com` with Docker). Nginx Proxy Manager, Traefik, Authelia or Cloudflare Access work too. Use a **subdomain**: Shiori does not work under a sub-path (`example.com/shiori`).

With Cloudflare (free plan): the 100 MB limit applies to each request, and Shiori only sends and receives pages of a few hundred KB, downloaded volumes included.

### The same address at home and away

At home there is no need to go through the Internet: point the same name at the local reverse proxy with **local DNS** (your router's setting, Pi-hole, AdGuard Home: `shiori.example.com` → the reverse proxy's local address). The reverse proxy needs a certificate valid for that name (Caddy, or Let's Encrypt with a DNS challenge). You keep a single address, so a single installed app and the same downloaded volumes, and home traffic stays at home.

## Installing the app on a tablet or phone

Shiori installs as an app (PWA), full screen, without the browser bar:

- **Android (Chrome)**: menu ⋮ → **Install app** (or "Add to home screen").
- **iPad, iPhone (Safari)**: Share button → **Add to Home Screen**.

Installing, offline reading and downloaded volumes need an **HTTPS** address (or `localhost`): browsers refuse them on `http://192.168…`. Use a reverse proxy as above.

## Updating

Back up the database first (below). The database updates itself when the new version starts.

- **Docker**: `docker compose pull && docker compose up -d`.
- **Standalone program**: stop Shiori, replace the program with the one from the new archive, start it again.

## Backing up

Everything that matters (libraries, reading progress, metadata, chosen covers) is in **a single file, `shiori.db`**. The cache needs no backup. Stop Shiori before copying the database, for a consistent copy:

- **Docker**:

  ```bash
  docker compose stop
  docker compose cp shiori:/data/shiori.db ./shiori-backup.db
  docker compose start
  ```

- **Standalone program**: stop Shiori and copy `shiori.db` from the data folder (table above).

To restore: stop Shiori, put the file back, start it again.

## Troubleshooting

- **"nom d'hôte non autorisé" (host name not allowed)**: you open Shiori by a name or address it does not know. Add it to `--allowed-host` / `SHIORI_ALLOWED_HOSTS`.
- **Docker, "unable to open database file"**: a host folder mounted on `/data` belongs to root. Keep the named volumes of the provided `compose.yaml`, or give that folder to the container's user (`user:` in `compose.yaml`).
- **A series is missing or split wrongly**: see [Organising your files](fonctionnel.md#ranger-ses-fichiers) and the scan log (Administration → History and options).
- **A volume is in error**: its menu → "Scan issues"; each code is explained in [the functional documentation](fonctionnel.md#codes-de-diagnostic) (French).
