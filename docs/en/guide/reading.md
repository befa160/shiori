# Reading

## The reader

The reader fills the screen. Controls appear with a **tap in the middle** (or Tab on a keyboard).

![Full-screen reader with its controls shown: back, title, Display and page slider](/screens/en/ecran_lecteur.jpg)

| Gesture | Effect |
|---|---|
| Tap left or right, swipe | previous or next page, depending on the chosen direction |
| Arrows, Page Down, Space, Home, End | the same on a keyboard |
| Pinch (finger) | the browser's own zoom, gradual; you move around with a finger, and no gesture turns the page while it is enlarged |
| Double-click in the middle (mouse) | 2× zoom where you clicked; drag to move around, a click does not turn the page; double-click, Escape or turning the page leaves it |
| Slider in the controls | go to any page |

The **Display** panel sets, for the series:

- the **reading direction** (manga, comics, webtoon);
- the **layout**: single page, double page, or automatic (double in landscape); in single page, a double-page spread is split into two halves read in order;
- the direction to **turn pages** ("like the reading direction", or tap right / left to go forward), with a general default in the administration and in the account preferences.

![Reader Display panel: reading direction, turning pages, layout, spreads](/screens/en/ecran_lecteur_affichage.jpg)

The page you read is saved on the server and follows you from one device to another. At the end of a volume, Shiori offers the next one; if it had errors at the last scan, the button says so ("Next volume (has errors)"), and that volume shows its unreadable pages as errors.

A damaged page in the archive shows an **explicit error in its place**, never another image. If the file changed since the scan, the reader says so and asks for a new scan.

## Pages fitted to the screen

Pages are served at the width of the screen (in real pixels), in AVIF, and kept in cache on the server and on the device. A page narrower than the screen is served as is, never enlarged. The next four pages are preloaded (two on a slow connection or with data saver on), and the server prepares the next five in advance: flipping quickly no longer waits for each page to be encoded.

## Reading offline

A volume can be **kept on the device** to read without a network. From a series page: a volume's menu → **Download to read offline**, or **Download series** for all the missing volumes.

- Pages are downloaded one by one, at the screen size of this device: a volume weighs much less than its archive.
- **Downloaded volumes** (sidebar, administration, or the link in the "Offline" notice) lists the kept volumes, their size and the space available; that is where you open them without a network.
- The page read offline is sent to the server as soon as the network is back, with its time: it does not replace a page read since on another device.
- An interrupted download (app closed) resumes at the next launch; a network outage stops it, to be restarted.
- Install the app **from the browser you read in, at your usual address**, and open it once online before leaving. Another address or another browser (Samsung Internet instead of Chrome, for example) is another app, with its own storage. If offline reading cannot work on this installation, **Downloaded volumes** says so and downloads are refused.

![Downloaded volumes page: space used, a download in progress and a kept volume](/screens/en/ecran_telecharges.jpg)

Outside downloads, the app also keeps the pages already seen: going back or a short outage asks nothing from the network.

::: warning HTTPS
Installing the app, offline reading and downloaded volumes need an **HTTPS** address (or `localhost`). See [Network and HTTPS](/en/installation/network).
:::

## Reading statistics

**Statistics** (sidebar, or the **Account** page on phones and tablets) shows what **your account** has read: nobody else sees it, not even the administrator.

- **Totals since the start**: pages read, reading time, volumes finished, series finished (all their readable volumes read).
- **Last 12 weeks**: pages read each week (Monday to Sunday), with the time and the volumes finished; hover a column for its details, or open **Week by week** for the table.
- Days follow the device time zone: reading at 11:30 pm counts for that day.
- Moving more than 50 pages at once (the slider) does not count as pages read, except reading done offline and sent when the network is back; a break of 5 minutes or more does not count in the time.
- **Mark as read** stays out of the statistics: it is not reading.
- Before version 1.1, only finished volumes are known: they count on the day they were finished, without pages or time.

## Installing the app

Shiori installs like an app (PWA), full screen, without the browser bar:

- **Android (Chrome)**: menu ⋮ → **Install app**.
- **iPad, iPhone (Safari)**: Share button → **Add to Home Screen**.
