# Reading

## The reader

The reader fills the screen. Controls appear with a **tap in the middle** (or Tab on a keyboard).

<!-- [ecran_lecteur] -->

| Gesture | Effect |
|---|---|
| Tap left or right, swipe | previous or next page, depending on the chosen direction |
| Arrows, Page Down, Space, Home, End | the same on a keyboard |
| Double tap | 2× zoom where you tapped; turning the page leaves it |
| Slider in the controls | go to any page |

The **Display** panel sets, for the series:

- the **reading direction** (manga, comics, webtoon);
- the **layout**: single page, double page, or automatic (double in landscape); in single page, a double-page spread is split into two halves read in order;
- the direction to **turn pages** ("like the reading direction", or tap right / left to go forward), with a general default in the administration and in the account preferences.

<!-- [ecran_lecteur_affichage] -->

The page you read is saved on the server and follows you from one device to another. At the end of a volume, Shiori offers the next one.

A damaged page in the archive shows an **explicit error in its place**, never another image. If the file changed since the scan, the reader says so and asks for a new scan.

## Pages fitted to the screen

Pages are served at the width of the screen (in real pixels), in AVIF, and kept in cache on the server and on the device. A page narrower than the screen is served as is, never enlarged. The next two pages are preloaded.

## Reading offline

A volume can be **kept on the device** to read without a network. From a series page: a volume's menu → **Download to read offline**, or **Download series** for all the missing volumes.

- Pages are downloaded one by one, at the screen size of this device: a volume weighs much less than its archive.
- **Downloaded volumes** (sidebar, administration, or the link in the "Offline" notice) lists the kept volumes, their size and the space available; that is where you open them without a network.
- The page read offline is sent to the server as soon as the network is back.
- An interrupted download (app closed) resumes at the next launch; a network outage stops it, to be restarted.

<!-- [ecran_telecharges] -->

Outside downloads, the app also keeps the pages already seen: going back or a short outage asks nothing from the network.

::: warning HTTPS
Installing the app, offline reading and downloaded volumes need an **HTTPS** address (or `localhost`). See [Network and HTTPS](/en/installation/network).
:::

## Installing the app

Shiori installs like an app (PWA), full screen, without the browser bar:

- **Android (Chrome)**: menu ⋮ → **Install app**.
- **iPad, iPhone (Safari)**: Share button → **Add to Home Screen**.
