# Series

## Lists

### `GET /api/series`

All visible series, sorted by name (natural sort). `?library_id=` restricts to one library. Filtering and sorting happen on the client.

```json
[{
  "id": 7, "library_id": 1, "name": "One Piece",
  "alt_titles": ["ワンピース"], "authors": ["Eiichiro Oda"], "tags": ["Adventure", "Shōnen"],
  "volume_count": 3, "read_count": 1, "page_count": 612, "pages_read": 250,
  "reading_status": "in_progress", "publication_status": "ongoing",
  "created_at": "…", "last_read_at": "…", "to_read_at": null,
  "cover_page_id": "3f9c…", "cover_image_id": null, "resume_volume_id": "a41b…"
}]
```

| Field | |
|---|---|
| `name` | displayed title: edited, otherwise files, otherwise web sources, otherwise folder name |
| `reading_status` | `unread`, `in_progress`, `read`, from the account's progress |
| `publication_status` | `ongoing`, `completed`, `hiatus`, `cancelled`, or `null` |
| `cover_page_id` / `cover_image_id` | cover page ([image](/en/api/volumes#get-api-pages-id-image)), or a source's cover ([`/api/covers/{id}`](/en/api/volumes#get-api-covers-id)), which wins |
| `resume_volume_id` | volume that "Read" opens: the one in progress, otherwise the first unread, otherwise the first |
| `missing_volumes` | numbers of the volumes missing from 1 to the official count (`metadata.volume_count`), a "T03-05" file covering three numbers; `null` if that count is unknown or no file has a volume number (series in chapters). A `completed` series whose list is not empty is **incomplete** |
| `to_read_at` | when the account put the series in "To read" ([`PUT …/to-read`](#put-api-series-id-to-read)), otherwise `null`; the home page sorts the list on it |

### `GET /api/continue`

"Continue reading": one entry per started series, most recent first; the volume in progress, or the next one when a volume is finished. `?limit=`: 20 by default, 100 at most.

```json
[{ "series_id": 7, "series_name": "One Piece", "library_id": 1, "volume": { "volume_id": "a41b…", "kind": "volume", "number": 2, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T02.cbz", "progress": { "page_id": "…", "ordinal": 45, "page_count": 204, "finished": false, "updated_at": "…" } }, "updated_at": "…" }]
```

### `GET /api/me/new`

New volumes in the series the account follows: volumes added since it followed the series or since the last "Mark all as seen", and never opened by the account. A moved or renamed file is not new (same volume), a modified file is (new volume); volumes already there before 1.3 never are. Once opened, a volume is never new again, even if its progress is cleared ("Mark as unread"). Series go from the most recently added to the oldest, each in volume order. Session only, like all of `/api/me`.

```json
[{ "series_id": 7, "series_name": "One Piece", "library_id": 1, "volume": { "volume_id": "c07e…", "kind": "volume", "number": 108, "…": "…" }, "added_at": "2026-11-26T09:12:44.512Z" }]
```

### `POST /api/me/new/seen`

"Mark all as seen": no volume already there is new for the account anymore. **Response 204.**

## A series

### `GET /api/series/{id}`

```json
{
  "id": 7, "library_id": 1, "name": "One Piece", "folder": "One Piece",
  "metadata": { "title": "One Piece", "alt_titles": [], "summary": "…", "authors": [{ "name": "Eiichiro Oda", "story": true, "art": true }],
    "genres": ["Adventure"], "tags": [], "year": 1997, "publisher": "Viz", "age_rating": null,
    "publication_status": "ongoing", "volume_count": null, "links": [], "cover_page_id": null, "cover_image_id": null,
    "main_source": null, "right_to_left": true },
  "locked": ["summary"],
  "sources": [],
  "publication_status": "ongoing",
  "settings": { "reading_mode": "rtl", "page_layout": "auto", "page_turn": "reading", "reading_mode_override": null, "page_layout_override": null, "page_turn_override": null },
  "followed": false, "to_read": false, "collections": [{ "id": 2, "name": "Shōnen" }], "resume_volume_id": "a41b…", "missing_volumes": null, "page_count": 612, "pages_read": 250,
  "volumes": [{ "volume_id": "a41b…", "kind": "volume", "number": 1, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T01.cbz", "title": "Romance Dawn", "progress": { "…": "…" } }]
}
```

- `metadata`: merged view, in the account's language ([metadata](/en/guide/metadata)).
- `locked`: edited fields, that sources no longer replace.
- `sources`: state of the web sources ([metadata](/en/api/metadata)).
- `settings`: reading settings applied for the account; `*_override`: what the series imposes (`null`: library default, or global default for `page_turn`).
- `followed`: the account follows the series ([new volumes](#get-api-me-new)).
- `to_read`: the series is in the account's "To read" list.
- `collections`: [collections](#collections) that hold it, by name.
- `volumes`: in reading order; `kind`: `volume`, `chapter`, `special`, `unknown`; `progress` missing if the volume was never opened.

### `PATCH /api/series/{id}` <Badge text="admin" />

Edits metadata. Each field given is **edited and locked** (`null` included); `unlock` hands fields back to the sources.

```json
{ "summary": "The series summary…", "genres": ["Adventure", "Comedy"], "unlock": ["title"] }
```

Fields: `title`, `alt_titles`, `summary`, `authors`, `genres`, `tags`, `year`, `publisher`, `age_rating`, `publication_status`, `volume_count` (official number of volumes, positive integer), `links`, `cover_page_id` (a page of the series), `cover_image_id`, `main_source`. **Response 200**: the series detail. **Errors**: 400 `invalid_body` (unknown field, invalid value, page of another series).

### `PATCH /api/series/{id}/settings`

The series' reading settings for the account; `null` goes back to the default.

```json
{ "reading_mode": "vertical", "page_layout": null, "page_turn": "ltr" }
```

`reading_mode`: `rtl`, `ltr`, `vertical`; `page_layout`: `single`, `double`, `auto`; `page_turn`: `reading`, `ltr`, `rtl`. **Response 200**: the settings applied.

### `PUT /api/series/{id}/read`

"Mark the series as read", for the account: each readable volume goes to its last page (a volume already read keeps the date it was first read). **Response 200**: the series detail. **Error**: 404 `not_found`.

### `DELETE /api/series/{id}/read`

"Mark the series as unread", for the account: the progress of all its volumes is cleared. **Response 200**: the series detail. **Error**: 404 `not_found`.

### `PUT /api/series/{id}/follow`

Follows the series for the account: volumes added afterwards show up in [new volumes](#get-api-me-new); those already there do not. Following twice changes nothing. **Response 204.** **Error**: 404 `not_found`.

### `DELETE /api/series/{id}/follow`

Stops following the series. **Response 204.** **Error**: 404 `not_found`.

### `PUT /api/series/{id}/to-read`

Puts the series in "To read", for the account; if it is already there, the date it was added does not change. It leaves on its own as soon as one of its pages is read, or it is marked read. **Response 204.** **Error**: 404 `not_found`.

### `DELETE /api/series/{id}/to-read`

Removes the series from "To read". **Response 204.** **Error**: 404 `not_found`.

## Collections

Shared collections: a name and some series, managed by the administrator, read by every account. An account only sees the series of its libraries, and not a collection where it sees no series; the administrator sees them all, empty ones included.

### `GET /api/collections`

By name. `series_ids`: the series the account can see; titles, covers and sorting come from the [series list](#get-api-series).

```json
[{ "id": 2, "name": "Shōnen", "series_ids": [7, 12] }]
```

### `POST /api/collections` <Badge text="admin" />

```json
{ "name": "Shōnen" }
```

Name without surrounding spaces, 1 to 100 characters, unique (case-insensitive, for ASCII letters only). **Response 201**: the collection, empty. **Errors**: 400 `invalid_name`, 409 `name_taken`.

### `PATCH /api/collections/{id}` <Badge text="admin" />

Renames: `{ "name": "Nekketsu" }`. **Response 200**: the collection. **Errors**: 400 `invalid_name`, 404 `not_found`, 409 `name_taken`.

### `DELETE /api/collections/{id}` <Badge text="admin" />

Deletes the collection, not its series. **Response 204.** **Error**: 404 `not_found`.

### `PUT /api/collections/{id}/series/{series_id}` <Badge text="admin" />

Adds a series (already there: nothing changes). **Response 200**: the collection. **Error**: 404 `not_found` (unknown collection or series).

### `DELETE /api/collections/{id}/series/{series_id}` <Badge text="admin" />

Removes a series. **Response 200**: the collection. **Error**: 404 `not_found`.
