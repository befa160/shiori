# Quick start

Shiori is a single program: it reads your folders of manga, comics and webtoons and serves them to a web app. Two ways to install it:

- **[Docker](#with-docker)**: the simplest on a NAS, a server or a Raspberry Pi.
- **[The standalone program](#with-the-standalone-program)**: Linux, Windows or macOS, with nothing else to install.

By default, Shiori can only be reached from the device it runs on; to open it to your network or the Internet, see [Network and HTTPS](/en/installation/network).

## With Docker

You need Docker and Docker Compose ([install Docker](https://docs.docker.com/get-started/get-docker/)).

1. Create a `shiori` folder and put this `compose.yaml` in it (the same as in [the repository](https://github.com/befa160/shiori/blob/main/compose.yaml)):

   ```yaml
   services:
     shiori:
       image: ghcr.io/befa160/shiori:latest
       restart: unless-stopped
       ports:
         - "127.0.0.1:7878:7878"   # "7878:7878" for the local network
       environment:
         SHIORI_ALLOWED_HOSTS: localhost
       volumes:
         - shiori-data:/data     # database, backups in /data/backups
         - shiori-cache:/cache   # resized pages, rebuilt on its own
         - /srv/manga:/libraries/manga:ro

   volumes:
     shiori-data:
     shiori-cache:
   ```

   Replace `/srv/manga` with the real folder. The right-hand part (`/libraries/manga`) is the path seen by Shiori: it is the one you will give in the interface. `:ro` makes it read-only. Add one line per folder (comics, webtoons…).

2. Start Shiori:

   ```bash
   docker compose up -d
   ```

3. Open <http://localhost:7878> and go to [first steps](#first-steps).

The image (`distroless`, amd64 and arm64) runs without administrator rights. Keep the **named volumes**: a host folder created by Docker belongs to root, and Shiori could not write to it.

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
   - **Windows**: double-click `shiori.exe`. On first launch Windows may show "Windows protected your PC": **More info**, then **Run anyway**. Shiori opens its interface in your browser; the black window stays open while it runs (closing it stops Shiori). Closed the interface by mistake? Double-click again: it reopens it.
   - **Linux**: `./shiori` in a terminal (or a double-click); the interface opens in the browser.
   - **macOS**: the program is not signed by Apple. Once, in a terminal: `xattr -d com.apple.quarantine shiori`, then `./shiori`.

3. Otherwise, open <http://localhost:7878>.

Without options, Shiori keeps its data in your system's usual folder:

| System | Database (back it up) | Cache (rebuilt on its own) |
|---|---|---|
| Linux | `~/.local/share/shiori/` | `~/.cache/shiori/` |
| Windows | `%APPDATA%\shiori\data\` | `%LOCALAPPDATA%\shiori\cache\` |
| macOS | `~/Library/Application Support/shiori/` | `~/Library/Caches/shiori/` |

Their exact location is shown at startup and in **Administration → Data**. To move them: [configuration](/en/installation/configuration).

## First steps

<!-- [ecran_installation] -->

1. **Create the administrator account.** On first launch, the interface shows the **Setup** page: an account name, a password (8 characters at least) and the **setup token**, printed at startup in Shiori's window or in `docker compose logs shiori`. It changes at every launch. Without the interface: `shiori user add <name> --admin` (Docker: `docker compose exec shiori shiori user add <name> --admin`) gives a temporary password.
2. **Administration → Add a library**: a name, the full path of the folder (with Docker: the right-hand path, for example `/libraries/manga`) and its type (manga, comics or webtoon).
3. The **scan** starts on its own; its progress shows in the administration. A first scan reads every file in full: allow a few minutes for a large library. Afterwards, [folder watching](/en/guide/scans#folder-watching) scans what changes on its own.
4. Your series appear on the home page. Store your files **one folder per series**: see [Organizing your files](/en/guide/libraries#organizing-your-files).
5. **Create your family's accounts** in **Administration → Accounts** ([accounts](/en/guide/accounts)).

Optional: **Administration → Web metadata** completes your series from AniList, MangaDex and Wikipedia (off by default; only series titles are sent).

## Updating

The database updates itself when the new version starts; if its format changes, Shiori backs it up first.

- **Docker**: `docker compose pull && docker compose up -d`.
- **Standalone program**: stop Shiori, replace the program with the one from the new archive, start it again.
