# Métadonnées

Sources web : `anilist`, `mangadex`, `wikipedia` (identifiant `langue:Titre de l'article`, par exemple `fr:One Piece` ; la recherche se fait dans la langue du compte). Toutes ces routes sont **admin** ; les sources doivent être autorisées dans les [réglages](/api/settings) (`metadata_online`), sinon 409 `metadata_disabled`.

État d'une source pour une série (`sources` du [détail d'une série](/api/series#get-api-series-id)) :

```json
{ "source": "mangadex", "external_id": "a1c7c817-…", "matched_by": "auto", "checked_at": "…", "last_error": null, "unlinked": false, "candidates": [] }
```

`matched_by` : `auto` (identification) ou `user` ; `unlinked` : déliée par l'utilisateur (l'identification automatique ne la relie plus) ; `candidates` : fiches à confirmer ; `last_error` : `offline`, `rate_limited`, `not_found`, `http_error`, `bad_response`.

### `GET /api/series/{id}/metadata/search`

`?source=anilist&q=One Piece`. **Réponse 200** : les fiches trouvées.

```json
[{ "id": "30013", "titles": [{ "lang": "ja-ro", "text": "ONE PIECE" }], "summaries": [], "authors": [], "genres": [], "tags": [], "year": 1997, "status": "ongoing", "links": [] }]
```

**Erreurs** : 400 `invalid_query` (`q` vide), 502 / 503 selon la source (`offline`, `rate_limited`, `http_error`, `bad_response`).

### `PUT /api/series/{id}/metadata/links/{source}`

Lie la série à une fiche.

```json
{ "external_id": "30013" }
```

**Réponse 200** : le détail de la série, métadonnées fusionnées à jour.

### `DELETE /api/series/{id}/metadata/links/{source}`

Délie la source : ses données ne comptent plus, et elle reste marquée `unlinked` (plus jamais reliée par l'identification automatique ; un `PUT` la relie). **Réponse 200** : le détail de la série.

### `POST /api/series/{id}/metadata/refresh`

Relit les fiches liées. **Réponse 200** : le détail de la série.

### `POST /api/series/{id}/metadata/cover`

Prend la couverture de la fiche liée chez une source (téléchargée et gardée par le serveur).

```json
{ "source": "anilist" }
```

### `GET /api/metadata/thumbnail/{source}/{id}`

Vignette d'un candidat, relayée par le serveur (le navigateur ne contacte jamais la source).

### `GET /api/admin/metadata`

```json
{ "online": true, "sources": ["mangadex", "anilist", "wikipedia"], "identify": { "running": false, "done": 40, "total": 40, "stopped": null }, "issues": [{ "series_id": 9, "series_name": "Blame!", "source": "anilist", "candidates": 2, "last_error": null }] }
```

`sources` : ordre appliqué ; `issues` : séries à vérifier (fiches à confirmer) ou en échec.

### `POST /api/admin/metadata/identify`

Lance l'identification des séries pas encore liées. **Réponse 202**. **Erreurs** : 409 `metadata_disabled`, 409 `already_running`.
