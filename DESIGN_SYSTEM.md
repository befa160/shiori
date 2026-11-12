# Design system de Shiori

Les règles visuelles de l'interface hors lecteur (bibliothèque, séries, administration). Le lecteur, plein écran et immersif, suit ses propres règles (CLAUDE.md, « Direction UI/UX »), mais reprend les mêmes couleurs, le même focus et les mêmes cibles tactiles.

Source de vérité du code : [`web/src/index.css`](web/src/index.css) (couleurs) et [`web/src/components/ui/`](web/src/components/ui/) (composants shadcn adaptés). Ce document dit **quoi utiliser et pourquoi** ; une règle nouvelle s'ajoute ici dans la PR qui l'introduit.

## Principes

1. **Les couvertures d'abord** : l'interface est sombre et discrète pour que les couvertures portent la couleur.
2. **Tablette d'abord** : cibles de 44 px, pas d'information réservée au survol, retour visuel au toucher.
3. **L'ambre a un sens** : action principale, état actif, progression. Rien d'autre.
4. **Rien n'est caché en silence** (principe 4 du projet) : une erreur s'affiche à sa place, avec son code traduit et une action.
5. **Aucune couleur en dur** dans les composants : toujours une couleur du thème (`bg-card`, `text-muted-foreground`…). Les valeurs hexadécimales ne vivent que dans `index.css`.

## Couleurs

Thème sombre unique (`color-scheme: dark`, classe `dark` fixe sur `<html>` : un OS en clair ne change rien).

| Rôle | Variable / classe | Valeur | Usage |
|---|---|---|---|
| Fond | `background` | `#121212` | fond de page |
| Surface | `card`, `popover` | `#1E1E1E` | cartes, panneaux, menus, sidebar, barre d'onglets |
| Survol / surface 2 | `accent`, `muted`, `secondary` | `#2A2A2A` | survol, bouton secondaire, squelettes sur surface, piste de progression |
| Texte | `foreground` | `#F4F4F5` | texte principal |
| Texte secondaire | `muted-foreground` | `#A1A1AA` (zinc-400) | auteur, compteurs, aide |
| Ambre | `primary` | `#E5A00D` | action principale, état actif, barre de progression, focus (`ring`) |
| Texte sur ambre | `primary-foreground` | `#121212` | libellé d'un bouton ambre |
| Erreur (texte) | `destructive` | `#F87171` | messages d'erreur |
| Erreur (fond) | `destructive-fill` | `#B91C1C` | bouton destructif, badge « Erreur » (texte blanc) |
| Bordure décorative | `border` | `#2A2A2A` | séparateurs |
| Bordure de champ | `input` | `#71717A` | contour des champs (élément non textuel) |

**L'ambre est `#E5A00D` exact, pas l'`amber-500` de Tailwind (`#F59E0B`)** : écrire `bg-primary`, jamais `bg-amber-500`.

### Contrastes mesurés (WCAG 2.2)

| Premier plan | sur `#121212` | sur `#1E1E1E` | sur `#2A2A2A` | Seuil |
|---|---|---|---|---|
| Texte `#F4F4F5` | 17,0 | 15,2 | 13,1 | 4,5 |
| Secondaire `#A1A1AA` | 7,3 | 6,5 | 5,6 | 4,5 |
| Ambre `#E5A00D` | 8,4 | 7,4 | 6,4 | 4,5 (texte), 3 (focus, barre) |
| Erreur `#F87171` | 6,8 | 6,0 | 5,2 | 4,5 |
| Bordure de champ `#71717A` | 3,9 | 3,4 | 3,0 | 3 |

Autres paires : `#121212` sur ambre 8,4:1 ; blanc sur `#B91C1C` 6,5:1. Le texte secondaire tombe à 4,1:1 sur `#3F3F46` (zinc-700) : **pas de texte secondaire sur une surface plus claire que `#2A2A2A`**.

## Typographie

- Pile système de Tailwind (`font-sans`), aucune police web : hors ligne, rapide, rien chargé chez un tiers.
- Échelle (classes Tailwind) :

| Rôle | Classes |
|---|---|
| Titre de page (`h1`) | `text-2xl font-semibold` ; tronqué `line-clamp-2 break-words` s'il vient des données |
| Titre de section (`h2`) | `text-lg font-semibold` |
| Corps | `text-base` (16 px : pas de zoom automatique d'iOS dans les champs) |
| Titre de carte | `text-sm font-medium line-clamp-2` |
| Secondaire, compteurs | `text-sm text-muted-foreground` ; chiffres alignés `tabular-nums` dans les tableaux |
| Badge | `text-xs font-medium` |

- Nombres, dates et pluriels par `Intl` (via i18n), jamais formatés à la main.
- Texte long (aide, journaux) : 75 caractères par ligne au plus (`max-w-prose`).

## Espacements, formes, profondeur

- Grille de 4 px (échelle Tailwind). Les valeurs courantes : `gap-1` (icône + texte serrés), `gap-3` / `space-y-3` (éléments d'une liste ou d'une grille), `gap-4` / `space-y-4` (blocs d'un écran), `space-y-8` (sections d'un écran), `p-4` (intérieur d'une carte), `px-4` (marge latérale mobile), `lg:px-8` (bureau).
- Rayon : `rounded-md` (6 px) partout, couvertures comprises ; `rounded-full` pour les barres de progression hors couverture et les pastilles.
- Profondeur par la **surface**, pas par l'ombre : fond → `card` → `accent`. Ombre seulement pour ce qui flotte (menus, fenêtres, notifications).
- Pas de bordure autour des cartes ; un séparateur `border-border` entre groupes si l'espace ne suffit pas.

## Mise en page

### Cadre et breakpoints (Q18)

| Largeur | Navigation | Exemple |
|---|---|---|
| `< lg` (< 1024 px) | barre d'onglets fixe en bas, 4 onglets au plus, au-dessus de `env(safe-area-inset-bottom)` ; le contenu réserve sa hauteur en bas | mobile 390 px, tablette 8" en portrait (800 px) |
| `≥ lg` | sidebar fixe à gauche (16 rem, réductible en icônes à 3,75 rem : entrée de 44 px et ses marges), recherche globale en tête | tablette en paysage (1280 px), bureau |

- Zones sûres : utilitaires `pt-safe`, `pb-safe`, `pl-safe`, `pr-safe` de `index.css` (absents de Tailwind) ; quand un élément a déjà une marge, prendre la plus grande des deux (`max()`), pas la somme ni l'une à la place de l'autre.
- Contenu : `mx-auto max-w-screen-2xl`, `px-4 lg:px-8`. Pas de défilement horizontal de la page.
- Page de détail : bouton « Retour » explicite (`BackLink`) en haut à gauche, vers l'écran parent (la barre du navigateur est masquée en PWA).

### Grille de couvertures

Une seule règle, sans breakpoint : `grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3` (`lg:gap-4`). Cela donne 2 colonnes en 390 px, 5 en 800 px, 6 en 1280 px avec sidebar, 7 en 1440 px.

### Carrousel (« Continuer la lecture », étagères)

- Défilement natif horizontal : `flex overflow-x-auto snap-x snap-mandatory` ; chaque carte `snap-start shrink-0 w-36 lg:w-40`.
- Barre de défilement masquée (`scrollbar-width: none` et `::-webkit-scrollbar { display: none }`), défilement tactile et molette conservés ; marge latérale négative pour que les cartes défilent jusqu'au bord de l'écran.
- Au clavier : chaque carte est un lien atteignable par Tab, qui la fait défiler à l'écran. Pas de flèches gauche / droite obligatoires ; sur bureau, des boutons ronds optionnels peuvent s'ajouter au survol de l'étagère.
- Titre d'étagère en `h2` + lien « Tout voir » si l'étagère est tronquée.

## Composants

Ajoutés **par le MCP shadcn** (`get_add_command_for_items`), puis adaptés. Après chaque ajout :

1. l'import `cn` doit venir de `@/lib/utils` (le registre génère `import { cn } from "cn"` ; la règle oxlint `no-restricted-imports` le bloque) ;
2. les dépendances générées sont vérifiées (`class-variance-authority` a déjà manqué) ;
3. les couleurs utilisées par le composant existent dans le thème (test `theme.test.ts`) ;
4. tailles tactiles relevées (ci-dessous) et liste de contrôle d'audit du MCP passée.

### Boutons (`Button`)

| Variante | Usage |
|---|---|
| `default` (ambre) | **une seule** action principale par écran ou fenêtre (« Lire », « Scanner maintenant », « Créer ») |
| `secondary` | actions courantes |
| `ghost` | barres d'outils, « Retour », actions de ligne |
| `outline` | filtres, choix secondaires |
| `destructive` | suppression, toujours dans une fenêtre de confirmation (AlertDialog) |

Tailles : `default` 44 px de haut, `icon` 44 × 44 ; `sm` / `xs` réservées aux barres denses du bureau (`lg:`). Un bouton icône a toujours un nom accessible (`aria-label` traduit).

### Carte de couverture (`CoverCard`, lot 4.4)

```
┌──────────────┐
│              │  couverture  aspect-[2/3] object-cover rounded-md bg-card
│   image      │
│              │
│▓▓▓▓▓▓░░░░░░░░│  progression h-1 collée en bas (tomes en cours seulement)
└──────────────┘
Titre sur deux lignes     text-sm font-medium line-clamp-2 (titre complet en `title`)
Auteur ou compteur        text-sm text-muted-foreground line-clamp-1
```

- Toute la carte est **un seul lien** (couverture + titre) ; la place de l'image est réservée par `aspect-[2/3]`, aucun saut au chargement.
- Image : miniature JPEG du serveur, `srcset` sur les largeurs fixes 160 / 320 / 480 et `sizes` d'après la grille, `loading="lazy"` sauf les cartes visibles au premier écran, `decoding="async"`, `alt=""` (le titre du lien suffit, pas de doublon pour les lecteurs d'écran).
- Survol (bureau, `@media (hover: hover)`) : zoom de l'image `scale-[1.03]` dans le cadre (`overflow-hidden`), liseré `ring-1 ring-white/15`, bouton rond « Lire » ambre en bas à droite de la couverture. Transition 200 ms.
- Toucher : `active:` assombrit légèrement (`active:brightness-90`) ; le bouton « Lire » n'est pas nécessaire au toucher, la carte ouvre la série et l'action principale y est.
- Clavier : `focus-visible` sur la carte montre le même liseré que le survol, et le bouton « Lire » devient atteignable par Tab.
- Badges sur la couverture (en haut à gauche, `text-xs`) : « Lu », « Erreur » (`destructive-fill`) ; le mot porte le sens, pas seulement la couleur.

### Couverture de repli

Quand la série n'a pas de couverture **ou** que l'image échoue (`onError`), même cadre `aspect-[2/3]` :

- fond en dégradé vertical de `hsl(<teinte> 35% 16%)` vers `#121212`, teinte tirée d'un hash du titre (stable : la même série a toujours la même couleur, deux séries voisines se distinguent) ;
- titre en `text-foreground font-semibold`, centré, `line-clamp-4`, `break-words`, dans une marge `p-3` ; auteur dessous en `text-muted-foreground` quand il existera (phase 5) ;
- contraste vérifié pour toute teinte : texte secondaire ≥ 4,75:1 sur la partie la plus claire du dégradé (16 % de luminosité au plus ; au-delà, il passe sous 4,5:1) ;
- `aria-hidden` : le titre est déjà donné par la carte.

### Barres de progression

- **Sur une couverture** : `h-1` collée en bas (`absolute inset-x-0 bottom-0`), indicateur ambre, piste `bg-black/80` (l’ambre reste à 5,6:1 même sur une image blanche ; à 60 % d’opacité il tombe à 2,6:1), sans arrondi.
- **Hors couverture** (page série, scan en cours) : composant `Progress` (Radix, `role="progressbar"` avec valeur), `h-1` ou `h-2`, `rounded-full`, toujours accompagné du texte « 45 / 183 » ou du pourcentage : la barre seule ne suffit pas.

### Navigation

- **Sidebar** (`≥ lg`, Sidebar shadcn) : surface `card`, entrées de 44 px, icône lucide + libellé ; entrée active `bg-accent text-primary` et `aria-current="page"` ; réduite, les icônes ont une infobulle et un nom accessible. État réduit gardé dans `localStorage`. Lien « Aller au contenu » en premier arrêt du clavier, visible au focus.
- **Barre d'onglets** (`< lg`) : `fixed inset-x-0 bottom-0`, surface `card` avec bordure haute `border-border`, 4 onglets égaux, icône au-dessus d'un libellé `text-xs` (jamais l'icône seule), onglet actif en ambre + `aria-current="page"`, hauteur 56 px + zone sûre.
- **Listes cliquables** (`Row`, administration) : ligne entière cliquable, 56 px au moins, 8 px entre deux lignes.

### Formulaires et fenêtres

- Champs : `Input` shadcn, 44 px, bordure `input`, libellé visible au-dessus (`Label`, jamais le seul placeholder), aide en `text-sm text-muted-foreground`, erreur sous le champ en `text-destructive` reliée par `aria-describedby` et `aria-invalid`.
- Erreur du serveur : traduite d'après son code stable (`errorText`), affichée près du champ concerné (`invalid_root` sous le chemin) ou en tête du formulaire.
- Fenêtres : `Dialog` pour créer ou modifier, `AlertDialog` pour une action irréversible (le texte dit ce qui est conservé : fichiers, progression). Focus piégé dans la fenêtre, Échap ferme, le focus revient au bouton d'origine.

### Notifications

Sonner (déjà monté hors des routes) : succès ou échec d'une action qui ne change pas l'écran (« Scan lancé », « Statut non enregistré »). Fermeture au bout de 5 s, au-dessus de la barre d'onglets et de la zone sûre. Une erreur qui empêche d'utiliser l'écran n'est jamais seulement une notification.

## États

| État | Rendu |
|---|---|
| Chargement | squelettes à la forme du contenu (couvertures `aspect-[2/3]`, lignes `h-14`), `aria-busy="true"` + texte « Chargement » pour les lecteurs d'écran ; pas de spinner plein écran |
| Vide | carte `bg-card p-4` qui dit **quoi faire** (« Aucune bibliothèque : créez-en une » + bouton vers l'administration) |
| Erreur | `role="alert"`, message traduit en `text-destructive`, bouton « Réessayer » ; pendant le nouvel essai, les squelettes reviennent (`QueryState`) |
| Désactivé | `opacity-50`, `pointer-events-none` ; la raison est écrite à côté si elle n'est pas évidente (« Un scan est déjà en cours ») |
| En cours (action) | bouton désactivé avec icône `Loader2` animée et libellé conservé |

États d'interaction : survol `hover:bg-accent` (seulement `@media (hover: hover)` pour les effets marqués), toucher `active:`, focus clavier `:focus-visible` (contour ambre 2 px décalé de 2 px, défini globalement : ne pas le retirer, ne pas ajouter d'anneau shadcn pâle).

## Mouvement

- Transitions de couleur 150 ms, de transformation (zoom de couverture) 200 ms, `ease-out`.
- Pas d'animation décorative ni de défilement automatique des carrousels.
- `prefers-reduced-motion: reduce` : transitions et animations quasi nulles (règle globale de `index.css`).

## Accessibilité

- Contraste ≥ 4,5:1 pour le texte, ≥ 3:1 pour les contours de champs, focus et barres (tableau ci-dessus).
- Cibles tactiles ≥ 44 × 44 px, 8 px entre deux cibles voisines.
- Tout est faisable au clavier, dans l'ordre visuel ; aucun piège ; focus toujours visible.
- Un repère par écran : `<title>` d'onglet traduit, un seul `h1`, sections en `h2`.
- Sens jamais porté par la seule couleur (badges écrits, onglet actif avec `aria-current`).
- Images : `alt=""` quand le texte voisin les nomme, sinon un `alt` traduit.
- Annonces dynamiques (progression d'un scan, résultats de recherche) par une région `aria-live="polite"`, mise à jour au plus une fois par seconde.

## Textes

- Aucun texte en dur : tout passe par `web/src/i18n/fr.ts` (source) et `en.ts` (typé sur le français).
- Ton : court, direct, à l'infinitif pour les actions (« Scanner maintenant »), une phrase qui dit quoi faire pour les états vides et les erreurs.
- Les libellés n’ont pas de largeur fixe (l’anglais et le français n’ont pas la même longueur) ; on ne tronque que les données (titres), jamais une action ni un message d’erreur.

## Vérification d'un lot d'interface

1. Skill UI/UX Pro Max avant de coder, et sa liste de contrôle à la fin.
2. Composants par le MCP shadcn, liste d'audit du MCP.
3. Données de test avec des titres très longs, des couvertures manquantes, un tome en erreur.
4. Captures Playwright en 390×844, 800×1280, 1280×800 et 1440×900 (binaire release), en français et en anglais pour les écrans à beaucoup de libellés ; défauts d'alignement corrigés.
5. Parcours au clavier seul.
