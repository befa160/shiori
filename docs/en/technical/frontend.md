# Frontend

- **React 19**, TypeScript, Vite, Tailwind, shadcn/ui components (Radix), TanStack Query for data, React Router.
- **i18n**: `react-i18next`, no hard-coded text; `fr.ts` is the source, `en.ts` is typed on it (a missing key breaks the build). API errors and scan codes translated from their stable code.
- **Themes**: CSS tokens in shadcn format in `index.css`, one `[data-theme]` block per theme (dark, light, OLED black, sepia), applied by `src/lib/theme.ts`; contrasts measured by `theme.test.ts`. Rules: [DESIGN_SYSTEM.md](https://github.com/befa160/shiori/blob/main/DESIGN_SYSTEM.md).
- **Navigation**: sidebar from the `lg` breakpoint, bottom tab bar below; safe areas (`env(safe-area-inset-*)`) for the PWA.

## Reader

- Views computed from the scan's dimensions (double pages, halves of spreads): the reader infers nothing on the fly.
- Preloads the next two views and the previous one (measured on slow 4G).
- Virtualized webtoon (`@tanstack/react-virtual`), each strip's space reserved from its dimensions.
- Progress saved 1 s after the last change and on close.
- An erroneous page shows the translated API message in its place.

## PWA and offline

- **Manifest** requested with credentials (reverse proxy with a password).
- Hand-written **service worker** (`public/sw.js`): pages and covers in a bounded cache, `assets/` files kept, navigations never intercepted online (the browser keeps control of the proxy's authentication), last interface served offline, the rest of the API always asked from the server. Only 200 responses are kept.
- **Downloaded volumes** (`src/offline/`): a volume = the JSON responses the reader reads and each page at the screen size in portrait, in the `shiori-offline-v1` cache, never emptied on its own; the service worker serves them without a network. Page by page: no request above a few MB. Offline progress kept on the device and sent when the network is back.
