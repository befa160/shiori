# Shiori — application web

React 19 + TypeScript + Vite, Tailwind 4 et shadcn/ui (composants copiés dans `src/components/ui`, ajoutés au besoin). Le binaire `shiori` l'embarquera (phase 3, lot 3.2).

```bash
npm ci
npm run dev        # http://localhost:5173, `/api` relayé vers un `shiori serve` local (127.0.0.1:7878)
npm run lint       # oxlint
npm run coverage   # Vitest, seuils de 80 % des lignes et des branches
npm run build      # vérification de types puis build dans dist/
```

- `src/api/` : client typé de l'API JSON (`/api`), erreurs avec leur code stable.
- `src/pages/` : écrans ; `src/routes.tsx` : routes React Router.
- `src/test/` : aides de test (rendu à une URL, `fetch` simulé).
- Thème sombre et couleurs : `src/index.css` (direction UI/UX de `CLAUDE.md`).
