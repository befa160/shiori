# Scans, watching and health

## The scan

A scan reads the files of a library, checks every page (size, CRC, header, dimensions) and updates the database. It is the only source of truth: the reader only shows what the scan established.

It starts when a library is created, when a file changes (see watching below), and on request: the button next to each library in the sidebar, or **Administration → History and options**, with two options:

- **Deep check**: fully decodes every image (slow, but finds damaged images);
- **Read every file again**: reads even the files Shiori believes unchanged (usually, size and date are enough).

A first scan reads every file in full; the next ones only read what changed. Only one scan runs at a time.

## Folder watching

The folders of every library are **watched**: a file added, replaced or removed starts, **30 seconds** after the last change (the time for a copy to finish), a scan of the affected series folder only.

- At startup, every watched library is rescanned to catch up with what changed while Shiori was off.
- A **full scan** of all libraries also runs once a day (configurable: every hour, every 6 hours, never). On a network share (SMB, NFS) or with Docker Desktop, changes made from another machine are not reported; this scan catches up with them.
- If a library's folder disappears (unplugged drive, unmounted share), nothing is removed from the library.

<!-- [ecran_admin_bibliotheques] -->

Watching is turned off per library (**Administration → Libraries**, "Watch folders" switch); the full scan interval is set in the same place.

## History and logs

<!-- [ecran_scans] -->

**Administration → History and options** shows the current scan (progress, file being read, cancellation) and the history of passes, filtered by status and library. Each pass keeps its log (files read, problems, errors) with a link to each volume concerned, and says where it came from: you, folder watching or the periodic full scan. Passes are kept for 30 days (option `--scan-log-days`).

## Library health

<!-- [ecran_sante] -->

**Administration → Library health** gives the overview:

- for each library, the number of **healthy** volumes, with **warnings**, with **errors**, and of **unreadable** files;
- the date of the last **full check**, and a **Full check** button (deep check of the whole library, the only thing that updates this date);
- the list of files **to repair**, filtered by library and problem, with each problem explained ([diagnostic codes](/en/guide/diagnostics)) and a link to the volume.

## On the command line

The same check exists without a server:

```bash
shiori scan /srv/manga --deep
```

Options: `--deep` (deep check), `--rehash` (read even unchanged files), `--jobs N` (files in parallel), `--json <file|->` (full JSON report, `-` for standard output), `--db <file>` or `--no-db` (no database, report only), `--fail-on error|warning` (severity that gives exit code 1, `error` by default). Exit code: 0 ok, 1 problem beyond `--fail-on`, 2 usage or fatal error. Help: `shiori scan --help`.
