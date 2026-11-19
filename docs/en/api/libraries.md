# Libraries

A library:

```json
{ "id": 1, "name": "Manga", "root": "/libraries/manga", "kind": "manga", "watch": true, "series_count": 42, "volume_count": 318 }
```

`kind`: `manga`, `comics` or `webtoon` (default reading direction). `root`: absolute path, `/` separators, no trailing `/`. `watch`: folders watched.

### `GET /api/libraries`

Libraries visible to the account (all of them for an administrator), sorted by name.

### `POST /api/libraries` <Badge text="admin" />

```json
{ "name": "Manga", "root": "/libraries/manga", "kind": "manga", "share": false }
```

`kind`: `manga` by default. `share`: opens it to existing readers (otherwise only visible to administrators). Watching is on. The scan is not started: call [`POST /api/admin/scans`](/en/api/scans#post-api-admin-scans).

**Response 201**

```json
{ "id": 1, "name": "Manga", "root": "/libraries/manga", "kind": "manga" }
```

**Errors**: 400 `invalid_name` (empty name), 400 `invalid_root` (not an absolute path to an accessible folder), 409 `name_taken`, 409 `root_overlaps` (folder containing another library, or contained in one).

### `PATCH /api/libraries/{id}` <Badge text="admin" />

Turns folder watching on or off.

```json
{ "watch": false }
```

**Response 204**. **Errors**: 400 `invalid_body`, 404 `not_found`.

### `DELETE /api/libraries/{id}` <Badge text="admin" />

Deletes the library, its series and its locations. Files are never touched; volumes and their progress stay known (a library recreated on the same folder finds them again). **Response 204**.

### `GET /api/libraries/{id}/series`

Series of a library, sorted by name (natural sort); like [`GET /api/series`](/en/api/series#get-api-series).
