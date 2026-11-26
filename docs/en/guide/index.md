# First steps

Shiori reads your folders of manga, comics and webtoons and serves them to a web app you open in the browser of your tablet, phone or computer. It never modifies your files.

This guide describes Shiori screen by screen. To install it: [quick start](/en/installation/). For how it works inside: [technical documentation](/en/technical/).

## In five minutes

1. **Install Shiori** ([Docker or standalone program](/en/installation/)) and open <http://localhost:7878>.
2. **Create the administrator account** on the **Setup** page, with the setup token shown at startup.
3. **Administration → Add a library**: a name, the folder, the type (manga, comics, webtoon).
4. The **scan** starts on its own; series appear on the home page.
5. Open a volume and read. The page you read is saved on the server.

## The interface

![Home: “Continue reading”, then the library shelves](/screens/en/ecran_accueil.jpg)

- **Large screens**: a collapsible sidebar (Home, Series, Search, your libraries, Documentation, Downloads, Administration, Account). On phones, the documentation opens from the **Account** page.
- **Phones and tablets in portrait**: a tab bar at the bottom (Home, Series, Search, Account).
- **Home**: "Continue reading" (the volume in progress in each series, or the next one when a volume is finished), "New volumes" (volumes added to the series you follow, until you open them or click "Mark all as seen"; hidden when there are none), then one shelf per library.
- **Series**: all series as covers, filtered by reading state (unread, in progress, read), publication status, library and **tags** (several tags = series that have them all), sorted by title, last read or recently added.
- **Search**: by title, alternative title, author or tag, ignoring accents and case.

![Series page: search, filters (reading, publication, tags, library), sort and covers](/screens/en/ecran_series.jpg)

- **Series page**: cover, summary, authors, genres and tags (each leads to the series that have it), progress, a button to resume or start, a **Follow** button (volumes added afterwards show up in "New volumes"; those already there do not; for your account only, on all your devices), and the list of volumes. A volume's menu marks it read or unread, downloads it for offline reading and shows the problems of its last scan. The series menu (on its page, or on its card in lists) marks the **whole** series read or unread, for your account only; "unread" clears the progress of all its volumes, so it asks for confirmation. A series is "Read" as soon as everything readable is read: an unreadable volume (in error) does not hold it back, it stays flagged by its badge.

An amber bar at the bottom of a cover shows the progress of a volume in progress.

![Series page: cover, progress, Resume, Edit, Download series, Identify, summary and tags](/screens/en/ecran_serie.jpg)

## Next

- [Organize your files](/en/guide/libraries) so that each series is split correctly.
- [Read](/en/guide/reading), offline included.
- [Create the family's accounts](/en/guide/accounts).
- [Open Shiori to the tablet or the Internet](/en/installation/network).
