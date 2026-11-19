# Frontend

- **React 19**, TypeScript, Vite, Tailwind, composants shadcn/ui (Radix), TanStack Query pour les données, React Router.
- **i18n** : `react-i18next`, aucun texte en dur ; `fr.ts` est la source, `en.ts` est typé dessus (une clé manquante casse le build). Erreurs de l'API et codes de scan traduits d'après leur code stable.
- **Thèmes** : jetons CSS au format shadcn dans `index.css`, un bloc `[data-theme]` par thème (sombre, clair, noir OLED, sépia), appliqués par `src/lib/theme.ts` ; contrastes mesurés par `theme.test.ts`. Règles : [DESIGN_SYSTEM.md](https://github.com/befa160/shiori/blob/main/DESIGN_SYSTEM.md).
- **Navigation** : barre latérale à partir du breakpoint `lg`, barre d'onglets en bas en dessous ; zones sûres (`env(safe-area-inset-*)`) pour la PWA.

## Lecteur

- Vues calculées à partir des dimensions du scan (doubles pages, moitiés de doubles pages) : le lecteur n'infère rien à la volée.
- Préchargement des deux vues suivantes et de la précédente (mesuré sur 4G lente).
- Webtoon virtualisé (`@tanstack/react-virtual`), la place de chaque bande réservée d'après ses dimensions.
- Progression enregistrée 1 s après le dernier changement et à la fermeture.
- Une page en erreur affiche le message traduit de l'API à sa place.

## PWA et hors ligne

- **Manifeste** demandé avec les identifiants (reverse proxy avec mot de passe).
- **Service worker** fait main (`public/sw.js`) : pages et couvertures en cache borné, fichiers `assets/` gardés, navigations jamais interceptées en ligne (le navigateur garde la main sur l'authentification du proxy), dernière interface servie hors ligne, reste de l'API toujours demandé au serveur. Seules les réponses 200 sont gardées.
- **Tomes téléchargés** (`src/offline/`) : un tome = les réponses JSON que lit le lecteur et chaque page à la taille de l'écran en portrait, dans le cache `shiori-offline-v1`, jamais vidé seul ; le service worker les sert sans réseau. Page par page : aucune requête au-delà de quelques Mo. Progression hors ligne gardée sur l'appareil et envoyée au retour du réseau.
