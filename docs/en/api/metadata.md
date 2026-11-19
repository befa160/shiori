# Metadata

Web sources: `anilist`, `mangadex`. All these routes are **admin**; sources must be allowed in the [settings](/en/api/settings) (`metadata_online`), otherwise 409 `metadata_disabled`.

State of a source for a series (`sources` in the [series detail](/en/api/series#get-api-series-id)):

```json
{ "source": "mangadex", "external_id": "a1c7c817-…", "matched_by": "auto", "checked_at": "…", "last_error": null, "candidates": [] }
```

`matched_by`: `auto` (identification) or `user`; `candidates`: entries to confirm; `last_error`: `offline`, `rate_limited`, `not_found`, `http_error`, `bad_response`.

### `GET /api/series/{id}/metadata/search`

`?source=anilist&q=One Piece`. **Response 200**: the entries found.

```json
[{ "id": "30013", "titles": [{ "lang": "ja-ro", "text": "ONE PIECE" }], "summaries": [], "authors": [], "genres": [], "tags": [], "year": 1997, "status": "ongoing", "links": [] }]
```

**Errors**: 400 `invalid_query` (empty `q`), 502 / 503 depending on the source (`offline`, `rate_limited`, `http_error`, `bad_response`).

### `PUT /api/series/{id}/metadata/links/{source}`

Links the series to an entry.

```json
{ "external_id": "30013" }
```

**Response 200**: the series detail, merged metadata up to date.

### `DELETE /api/series/{id}/metadata/links/{source}`

Unlinks the source: its data no longer counts. **Response 200**: the series detail.

### `POST /api/series/{id}/metadata/refresh`

Reads the linked entries again. **Response 200**: the series detail.

### `POST /api/series/{id}/metadata/cover`

Takes the cover of the linked entry at a source (downloaded and kept by the server).

```json
{ "source": "anilist" }
```

### `GET /api/metadata/thumbnail/{source}/{id}`

A candidate's thumbnail, relayed by the server (the browser never contacts the source).

### `GET /api/admin/metadata`

```json
{ "online": true, "sources": ["mangadex", "anilist"], "identify": { "running": false, "done": 40, "total": 40, "stopped": null }, "issues": [{ "series_id": 9, "series_name": "Blame!", "source": "anilist", "candidates": 2, "last_error": null }] }
```

`sources`: applied order; `issues`: series to check (entries to confirm) or failing.

### `POST /api/admin/metadata/identify`

Starts identifying the series not linked yet. **Response 202**. **Errors**: 409 `metadata_disabled`, 409 `already_running`.
