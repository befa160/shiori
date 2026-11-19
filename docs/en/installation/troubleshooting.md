# Troubleshooting

| Symptom | What to do |
|---|---|
| **Forgotten password** | The administrator gives a new one in **Administration → Accounts**. For the administrator themselves: `shiori user password <name>` (Docker: `docker compose exec shiori shiori user password <name>`), which also closes their sessions. |
| **Lost two-factor phone** | A **recovery code** (given when turning it on, each works once). Otherwise, an administrator removes it (**Administration → Accounts**, or `shiori user totp-reset <name>`). |
| **"Too many attempts"** | After several wrong passwords, Shiori makes you wait, longer and longer (15 minutes at most). |
| **Setup token not found** | It is only asked while no account exists; it is printed at every start in Shiori's window or in `docker compose logs shiori`. |
| **"host name not allowed"** | You open Shiori with a name or address it does not know: add it to `--allowed-host` / `SHIORI_ALLOWED_HOSTS`. |
| **Docker, "unable to open database file"** | A host folder mounted on `/data` belongs to root. Keep the named volumes of `compose.yaml`, or give that folder to the container's user (`user:`). |
| **A series is missing or badly split** | See [Organizing your files](/en/guide/libraries#organizing-your-files) and the scan log (**Administration → History and options**). |
| **A volume is in error** | Its menu → **Scan issues**, or **Administration → Library health**; each code is explained in [diagnostic codes](/en/guide/diagnostics). |
| **An added file does not show up** | Watching waits for 30 s of calm after the last change. On a network share or with Docker Desktop, changes made elsewhere are not reported: start a scan, or wait for the periodic full scan. |
| **The scan fails "unmounted folder?"** | The library's folder is empty or unreadable while the database knows files in it: Shiori refuses to remove everything. Mount the disk or share again, then start again. |
| **The app does not install, no offline** | It needs an HTTPS address (or `localhost`): see [Network and HTTPS](/en/installation/network). |
| **Linux: "cannot be watched"** in the log | Too many folders for the inotify limit: raise `fs.inotify.max_user_watches` (`sysctl`). The periodic full scan keeps working anyway. |
