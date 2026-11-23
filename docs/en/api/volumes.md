# Volumes and pages

## Volumes

### `GET /api/volumes/{id}`

`id`: the volume identifier (hexadecimal BLAKE3 of the file).

```json
{
  "id": "a41b…", "size": 48213504, "format": "cbz", "status": "warning", "page_count": 204, "scanned_at": "…",
  "locations": [{ "path": "/libraries/manga/One Piece/One Piece T01.cbz", "library_id": 1, "series_id": 7 }],
  "issues": [{ "code": "duplicate_entry_identical", "severity": "warning", "entry": null, "message": "…" }],
  "settings": { "reading_mode": "rtl", "page_layout": "auto", "page_turn": "reading", "reading_mode_override": null, "page_layout_override": null, "page_turn_override": null },
  "progress": null
}
```

`status`: `ok`, `warning`, `error`; `format`: from the content, never the extension; `issues`: problems of the last scan ([codes](/en/guide/diagnostics)).

### `PATCH /api/volumes/{id}` <Badge text="admin" />

The volume's metadata, as for a series: a field given is edited and locked, `unlock` hands it back to the files. Fields: `title`, `summary`, `release_date`, `authors`. **Response 200**: the volume detail.

## Pages

### `GET /api/volumes/{id}/pages`

Pages in reading order, set by the scan. The reader infers nothing else.

```json
[{ "id": "3f9c0a…", "ordinal": 0, "width": 1600, "height": 2400, "media_type": "image/jpeg", "status": "ok" }]
```

`status`: `ok` or `broken` (page unreadable in the archive: its image answers an error).

### `GET /api/pages/{id}/image`

A page, by its identifier (never by an index). Without parameters: the original. With `w`: resized.

| Parameter | Values |
|---|---|
| `w` | 160, 320, 480, 640, 800, 1080, 1280, 1600, 2048 |
| `f` | `avif` (default), `jpeg`; requires `w` |

The page is never enlarged (narrower than `w`: the original); an AVIF page is served as is. Response with `Cache-Control: immutable` and an `ETag`; `If-None-Match` gives 304.

**Errors**:

| Status | Code | Case |
|---|---|---|
| 400 | `invalid_width`, `invalid_format` | parameter outside the list |
| 404 | `page_not_found` | unknown or not visible page |
| 409 | `volume_changed` | the file changed since the scan: rescan |
| 409 | `page_mismatch` | the archive no longer has this page in its place: rescan |
| 410 | `volume_unavailable` | no known file of this volume exists anymore |
| 422 | `page_broken` | page unreadable in the archive (see the volume's problems) |
| 500 | `page_corrupt`, `render_failed` | damaged content when reading, rendering failure |

Never another page instead: any inconsistency is an explicit error.

### `GET /api/covers/{id}`

A web source's cover kept by the server (`cover_image_id`), original or `?w=&f=` like a page.

## Progress

### `GET /api/volumes/{id}/progress`

```json
{ "page_id": "3f9c…", "ordinal": 45, "page_count": 204, "finished": false, "updated_at": "…" }
```

**Response 204** if the volume was never opened by this account.

### `PUT /api/volumes/{id}/progress`

Saves the page read. Allowed to API tokens.

```json
{ "page_id": "3f9c…", "read_at": "2026-11-19T08:12:00.000Z", "utc_offset": 60 }
```

`read_at`: optional, time of reading (ISO 8601) of a page read offline and sent later; missing: now, and a future time counts as now. The most recent page wins: a page read offline does not replace the one read since on another device.

`utc_offset`: optional, the device time zone in minutes east of UTC (UTC by default, capped at ±840), which gives the day counted by the [statistics](account#reading-statistics). `stats`: optional, `false` for a save that is not reading (“Mark as read”) and stays out of the statistics.

**Response 200**: the progress kept. `finished` becomes true as soon as the last page is reached, even by an older page. **Errors**: 400 `invalid_body` (including a `read_at` that is not a date), 404 `not_found`, 422 `page_not_in_volume`.

### `DELETE /api/volumes/{id}/progress`

"Mark as unread". **Response 204**.
