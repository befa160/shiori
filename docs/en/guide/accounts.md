# Accounts and security

Each person signs in with their own account: their progress, their settings, the libraries open to them.

| Role | Can |
|---|---|
| **Administrator** | everything: libraries, scans, metadata (shared by all), accounts, default settings, backups; sees every library |
| **Reader** | read the libraries open to them, with their own progress, reading settings and preferences |

## Creating accounts

- **First account**: the **Setup** page while no account exists, with the **setup token** shown when Shiori starts (it changes at every launch and keeps the first comer on your network from taking over the instance). Without the interface: `shiori user add <name> --admin`.
- **Administration → Accounts**: create an account (name, role, open libraries), change its role and libraries, disable it, give it a new temporary password, sign out all its devices and tokens, remove its two-factor authentication, delete it (its progress goes with it, not the files or entries).
- A **temporary password** is shown only once and must be changed at the first sign-in.
- The last active administrator can be neither demoted, nor disabled, nor deleted.

## Signing in

Name and password (8 characters at least), then the two-factor code if it is on. After several wrong passwords, Shiori makes you wait before the next attempt, longer and longer (15 minutes at most), per account and per address. A session lasts **90 days after its last use**: the tablet stays signed in as long as it is used.

## Account page

<!-- [ecran_compte] -->

- **Preferences**: language, theme and page-turn direction for this account, on all its devices.
- **Password**: changing it signs out its other devices.
- **Signed-in devices**: each session with its last use; sign out a lost device.
- **Two-factor authentication**: on top of the password, a 6-digit code from an authenticator app (Aegis, Google Authenticator, 2FAS…). Turning it on asks for the password, shows a QR code (or a key to type), checks a first code and gives **10 recovery codes**, shown only once, each replacing the phone's code once. Recommended if Shiori is open to the Internet.
- **API tokens**: so that another program reads the account's libraries and saves its progress without the password. A token is shown only once, can expire (30 days, 90 days, 1 year or never) and only allows reading and progress. See [the API](/en/api/).
- **Sign out**: closes this device's session and clears the cached pages; also offers to delete downloaded volumes.

## Forgotten and lost

- **Forgotten password**: the administrator gives a new one in **Administration → Accounts**. For the administrator themselves: `shiori user password <name>`.
- **Lost phone**: a recovery code, otherwise an administrator removes two-factor authentication (**Administration → Accounts**, or `shiori user totp-reset <name>`).
