# Référence API

Shiori expose une API JSON sous `/api`. L'application web n'utilise rien d'autre : tout ce qu'elle fait, un autre logiciel peut le faire.

## Authentification

Toutes les routes exigent une session, sauf `GET /api/health` et `/api/auth/*`. Trois façons de s'authentifier :

| Moyen | Usage |
|---|---|
| Cookie `shiori_session` | navigateur, posé par `POST /api/auth/login` |
| `Authorization: Bearer <jeton de session>` | application native (`"client": "app"` à la connexion) |
| `Authorization: Bearer shiori_t_…` | [jeton d'API](/api/account#jetons-d-api) : lecture et progression seulement |

Un jeton d'API n'ouvre que les `GET` hors `/api/me` et `/api/admin`, et `PUT /api/volumes/{id}/progress` ; ailleurs, 403 `token_forbidden`. Avec un mot de passe provisoire, seules `/api/me` et `/api/me/password` répondent (403 `password_change_required`).

```bash
curl -H "Authorization: Bearer shiori_t_…" https://shiori.example/api/series
```

## Rôles

Les routes marquées **admin**, et toutes celles sous `/api/admin`, répondent 403 `forbidden` à un lecteur. Un lecteur ne voit que les bibliothèques qui lui sont ouvertes : le reste répond 404, sans dire si la ressource existe. Progression, réglages de lecture et langue des titres sont ceux du compte connecté.

## Erreurs

Toute erreur a la même forme, avec un **code stable** (que l'interface traduit) et un message lisible (en français) :

```json
{ "code": "scan_already_running", "message": "un scan est déjà en cours" }
```

| Statut | Codes courants |
|---|---|
| 400 | `invalid_body`, `invalid_query`, `invalid_id` |
| 401 | `unauthenticated` |
| 403 | `forbidden`, `token_forbidden`, `password_change_required`, `cross_site`, `invalid_host` |
| 404 | `not_found` |
| 405 | `method_not_allowed` |
| 409 | conflits propres à chaque route (`scan_already_running`, `name_taken`…) |
| 429 | `rate_limited` (en-tête `Retry-After`) |
| 500 | `internal` (détail dans le journal du serveur, jamais dans la réponse) |

Les codes propres à une route sont listés sur sa page.

## Conventions

- **JSON** en entrée et en sortie, `Content-Type: application/json`. Un champ inconnu dans un corps est refusé (`invalid_body`) quand la route le précise.
- **Mise à jour partielle** (`PATCH`, `PUT /api/settings`, `PUT /api/me`) : un champ absent est inchangé, `null` revient au défaut quand c'est permis.
- **Identifiants** : nombres pour les bibliothèques, séries, comptes et passes ; chaînes hexadécimales pour les tomes (BLAKE3, 64 caractères) et les pages (128 bits, 32 caractères). Ils ne changent jamais tant que le contenu ne change pas.
- **Dates** : RFC 3339 en UTC (`2026-11-18T21:57:23.374Z`).
- **Pagination** par curseur : `limit` (50 par défaut, 500 au plus) et `before` / `after` selon la liste.
- **Écritures** refusées si le navigateur annonce une requête qui ne vient pas de Shiori lui-même (`Sec-Fetch-Site` autre que `same-origin` ou `none` : un autre site, mais aussi un autre sous-domaine ou un autre port).
- **Stabilité** : à partir de la 1.0, l'API ne fait que s'enrichir au sein d'une version majeure ; un client ignore les champs qu'il ne connaît pas. Voir [ce qu'une mise à jour garantit](/installation/#ce-qu-une-mise-a-jour-garantit-a-partir-de-la-1-0).

## Santé du serveur

### `GET /api/health`

Sans authentification.

```json
{ "status": "ok", "version": "1.4.0" }
```

`version` : le numéro de la release installée (`1.0.0-rc.1` pour une préversion), sinon la version du code.

## Pages de la référence

| Page | Routes |
|---|---|
| [Authentification](/api/auth) | installation, connexion, double authentification, déconnexion, liens d'invitation |
| [Compte](/api/account) | compte connecté, préférences, mot de passe, appareils, double authentification, jetons d'API |
| [Comptes (admin)](/api/users) | gestion des comptes, invitations |
| [Bibliothèques](/api/libraries) | liste, création, surveillance, suppression |
| [Séries](/api/series) | listes, détail, modification, réglages de lecture, « Continuer la lecture », nouveautés, « À lire », collections |
| [Tomes et pages](/api/volumes) | détail, progression, pages, images, couvertures |
| [Métadonnées](/api/metadata) | sources web, recherche, liens, identification |
| [Scans](/api/scans) | lancer, suivre, annuler, historique, journaux |
| [Santé](/api/health) | état des bibliothèques, fichiers à réparer |
| [Réglages](/api/settings) | réglages de l'instance |
| [Sauvegardes](/api/backups) | stockage, sauvegardes, restauration |
