<h1>
  <picture>
    <source media="(prefers-color-scheme: light)" srcset="web/public/shiori-mark-light.svg" />
    <img src="web/public/shiori-mark.svg" alt="" width="40" height="40" align="absmiddle" />
  </picture>
  Shiori
</h1>

[![CI](https://github.com/befa160/shiori/actions/workflows/ci.yml/badge.svg)](https://github.com/befa160/shiori/actions/workflows/ci.yml)
[![Docs](https://github.com/befa160/shiori/actions/workflows/docs.yml/badge.svg)](https://befa160.github.io/shiori/)
[![Release](https://img.shields.io/github/v/release/befa160/shiori)](https://github.com/befa160/shiori/releases)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

**Lecteur de mangas, BD et webtoons auto-hébergé.** Un seul programme (binaire Rust ou image Docker) lit vos dossiers de CBZ, CBR et PDF et les sert à une application web installable, pensée d'abord pour la tablette. Vos fichiers ne sont jamais modifiés.

📖 **Documentation : <https://befa160.github.io/shiori/>** ([English](https://befa160.github.io/shiori/en/))

<!-- [ecran_readme] -->

## Ce que ça fait

- **Lire partout** : manga de droite à gauche, BD, doubles pages, webtoon en défilement continu ; la progression suit d'un appareil à l'autre ; application installable (PWA) et tomes téléchargés pour lire hors ligne.
- **Tous vos formats** : CBZ (zip64 compris), CBR (RAR 4 et 5, solide compris) et PDF, dont la planche s'affiche sans les marges de la page. Pages redimensionnées en AVIF à la taille de l'écran.
- **Une bibliothèque qui se tient à jour** : un dossier par série, les numéros de tomes et chapitres reconnus dans les noms, la surveillance des dossiers qui scanne seule ce qui change, un scan complet périodique pour les partages réseau.
- **Une bibliothèque en bonne santé** : chaque page vérifiée au scan, un tableau de bord des tomes sains, en avertissement ou en erreur, la liste de ce qu'il faut réparer, chaque problème expliqué.
- **Des fiches complètes** : ComicInfo.xml, `index.json`, propriétés des PDF, et si vous l'autorisez AniList et MangaDex ; vos modifications passent toujours en premier.
- **Toute la famille** : un compte par personne, bibliothèques ouvertes à la carte, double authentification, jetons d'API, sauvegardes automatiques et restauration depuis l'interface.
- **Chez vous** : Linux, Windows, macOS ou Docker (amd64, arm64) ; rien ne quitte votre réseau sans votre accord. Interface en français et en anglais, thèmes sombre, clair, noir OLED et sépia.

## Pourquoi

Les lecteurs existants affichent parfois la mauvaise page : un webtoon où la page 1 n'est plus la page 1, un manga qui répète la 4ᵉ de couverture pendant soixante pages. Ces bugs viennent d'un index de page qui ne correspond plus au contenu réel de l'archive (cache périmé, nombre de pages faux).

Shiori est conçu pour que ces défauts soient impossibles :

- **Chaque page a une identité stable** : un hash du fichier et du nom de l'entrée dans l'archive, jamais un simple numéro. Une adresse d'image désigne toujours la même image.
- **Le scan est la source de vérité** : l'ordre des pages, leurs dimensions et leur type sont établis une fois au scan et stockés en base. Le lecteur n'invente rien.
- **Une archive abîmée se voit** : une page illisible affiche une erreur explicite à sa place, jamais une autre image.
- **Les fichiers sources ne sont jamais modifiés.**

## Démarrage rapide

Avec Docker, ce `compose.yaml` (le même que [celui du dépôt](compose.yaml)) :

```yaml
services:
  shiori:
    image: ghcr.io/befa160/shiori:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:7878:7878"
    volumes:
      - shiori-data:/data
      - shiori-cache:/cache
      - /chemin/vers/mangas:/bibliotheques/mangas:ro

volumes:
  shiori-data:
  shiori-cache:
```

```bash
docker compose up -d
docker compose logs shiori   # le jeton d'installation
```

Puis <http://localhost:7878> : créez le compte administrateur avec le jeton d'installation, puis **Administration → Ajouter une bibliothèque** avec le chemin `/bibliotheques/mangas`. Le scan démarre tout seul.

Sans Docker : téléchargez le programme de votre système dans les [Releases](https://github.com/befa160/shiori/releases) et double-cliquez dessus ; il ouvre l'interface dans votre navigateur.

Ensuite : [ouvrir Shiori à la tablette ou à Internet](https://befa160.github.io/shiori/installation/network), [ranger ses fichiers](https://befa160.github.io/shiori/guide/libraries), [sauvegardes](https://befa160.github.io/shiori/installation/backup).

## Documentation

| | Français | English |
|---|---|---|
| Installation | [Démarrage rapide](https://befa160.github.io/shiori/installation/) | [Quick start](https://befa160.github.io/shiori/en/installation/) |
| Guide | [Utiliser Shiori](https://befa160.github.io/shiori/guide/) | [Using Shiori](https://befa160.github.io/shiori/en/guide/) |
| Technique | [Architecture](https://befa160.github.io/shiori/technical/) | [Architecture](https://befa160.github.io/shiori/en/technical/) |
| API | [Référence](https://befa160.github.io/shiori/api/) | [Reference](https://befa160.github.io/shiori/en/api/) |

Les sources de la documentation sont dans [`docs/`](docs/), l'avancement détaillé dans [ROADMAP.md](ROADMAP.md).

## In English

Shiori is a self-hosted manga, comics and webtoon reader: a single Rust binary (or Docker image) that reads your CBZ, CBR and PDF folders and serves them to an installable web app, designed tablet-first. Every page has a stable identity and is checked when scanned, so the reader never shows the wrong page. See the [English documentation](https://befa160.github.io/shiori/en/).

## Développement

Rust (axum, tokio, sqlx + SQLite) et React 19 + TypeScript + Tailwind + shadcn/ui, embarqué dans le binaire. Code portable (Linux, Windows, macOS), dépendances Rust pur de préférence, couverture de tests d'au moins 80 %.

```bash
cd backend && cargo test --workspace
cd web && npm ci && npm run dev
```

Commandes, règles et publication : [Contribuer](https://befa160.github.io/shiori/development/) ; règles complètes dans [CLAUDE.md](CLAUDE.md).

## Licence

[MIT](LICENSE)
