# Settings

## Appearance and language

- **Theme**: Dark (default), Light, OLED black, Sepia, or System (dark or light depending on the device). The reader always stays dark.
- **Language**: French or English; without a choice, the browser's.

Each account picks its own in **Account → Preferences** (the language also in the sidebar), saved on the server for all its devices. The administration sets those of accounts that chose nothing (**Administration → Interface language**, **Theme**, **Reading**).

## Administration

| Section | Setting |
|---|---|
| Libraries | add, scan, delete (files are never touched), folder watching, full scan interval |
| Library health | state of volumes, full check, files to repair |
| History and options | current scan, start with options, history and logs |
| Accounts | roles, open libraries, passwords, two-factor authentication |
| Reading | default page-turn direction |
| Web metadata | permission, order of sources, adult entries, identification |
| Language, Theme | account defaults |
| Data | location of the database and cache, backups |

## Backups

Shiori backs up its database every day (the 7 most recent kept), before each update of its format and before each restore. **Administration → Data** shows the backup folder and the list, and lets you back up right away, download, delete or **restore** a backup, from the list or a file, without stopping Shiori. After a restore, everyone signs in again. Details: [Backups](/en/installation/backup).

## Current limits

- Accounts are created by the administrator only: no invitation link yet, nor sign-in delegated to an authentication proxy (`Remote-User` header).
- One address per app installation: to use the local address at home and the public one elsewhere, see [local DNS](/en/installation/network#the-same-address-at-home-and-away).
- Web metadata comes from AniList, MangaDex, Wikipedia, MangaUpdates and Comic Vine only.
