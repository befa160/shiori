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
  "created_at": "…", "last_read_at": "…",
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

### `GET /api/continue`

"Continue reading": one entry per started series, most recent first; the volume in progress, or the next one when a volume is finished. `?limit=`: 20 by default, 100 at most.

```json
[{ "series_id": 7, "series_name": "One Piece", "library_id": 1, "volume": { "volume_id": "a41b…", "kind": "volume", "number": 2, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T02.cbz", "progress": { "page_id": "…", "ordinal": 45, "page_count": 204, "finished": false, "updated_at": "…" } }, "updated_at": "…" }]
```

## A series

### `GET /api/series/{id}`

```json
{
  "id": 7, "library_id": 1, "name": "One Piece", "folder": "One Piece",
  "metadata": { "title": "One Piece", "alt_titles": [], "summary": "…", "authors": [{ "name": "Eiichiro Oda", "story": true, "art": true }],
    "genres": ["Adventure"], "tags": [], "year": 1997, "publisher": "Viz", "age_rating": null,
    "publication_status": "ongoing", "links": [], "cover_page_id": null, "cover_image_id": null,
    "main_source": null, "right_to_left": true },
  "locked": ["summary"],
  "sources": [],
  "publication_status": "ongoing",
  "settings": { "reading_mode": "rtl", "page_layout": "auto", "page_turn": "reading", "reading_mode_override": null, "page_layout_override": null, "page_turn_override": null },
  "resume_volume_id": "a41b…", "page_count": 612, "pages_read": 250,
  "volumes": [{ "volume_id": "a41b…", "kind": "volume", "number": 1, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T01.cbz", "title": "Romance Dawn", "progress": { "…": "…" } }]
}
```

- `metadata`: merged view, in the account's language ([metadata](/en/guide/metadata)).
- `locked`: edited fields, that sources no longer replace.
- `sources`: state of the web sources ([metadata](/en/api/metadata)).
- `settings`: reading settings applied for the account; `*_override`: what the series imposes (`null`: library default, or global default for `page_turn`).
- `volumes`: in reading order; `kind`: `volume`, `chapter`, `special`, `unknown`; `progress` missing if the volume was never opened.

### `PATCH /api/series/{id}` <Badge text="admin" />

Edits metadata. Each field given is **edited and locked** (`null` included); `unlock` hands fields back to the sources.

```json
{ "summary": "The series summary…", "genres": ["Adventure", "Comedy"], "unlock": ["title"] }
```

Fields: `title`, `alt_titles`, `summary`, `authors`, `genres`, `tags`, `year`, `publisher`, `age_rating`, `publication_status`, `links`, `cover_page_id` (a page of the series), `cover_image_id`, `main_source`. **Response 200**: the series detail. **Errors**: 400 `invalid_body` (unknown field, invalid value, page of another series).

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
