# Health

**Admin** routes of the Library health page.

### `GET /api/admin/health`

```json
{
  "libraries": [
    { "id": 1, "name": "Manga", "ok": 300, "warning": 16, "error": 2, "unreadable": 1, "deep_checked_at": "2026-11-10T08:00:00.000Z" }
  ],
  "codes": [
    { "code": "crc_mismatch", "severity": "error", "files": 2 },
    { "code": "duplicate_entry_identical", "severity": "warning", "files": 12 }
  ]
}
```

- `ok`, `warning`, `error`: distinct volumes by the state of their last scan.
- `unreadable`: files without a reliable volume (unreadable before even their hash).
- `deep_checked_at`: end of the last `deep` pass completed on the whole library; `null`: never.
- `codes`: warnings and errors present, most severe then most frequent first, with the number of files concerned.

### `GET /api/admin/health/files`

Files to repair (volumes with warnings or errors, unreadable files), sorted by path.

| Parameter | |
|---|---|
| `library_id` | one library |
| `code` | a [diagnostic code](/en/guide/diagnostics) |
| `after` | path of the last file received (pagination) |
| `limit` | 50 by default, 500 at most |

```json
[{
  "path": "/libraries/manga/One Piece/One Piece T01.cbz", "library_id": 1, "volume_id": "a41b…", "series_id": 7, "status": "error",
  "issues": [{ "severity": "error", "code": "crc_mismatch", "entry_name": "003.png", "message": "…" }]
}]
```

`volume_id` and `series_id`: `null` for an unreadable file. Problems sorted, errors first. **Errors**: 400 `invalid_query` (unknown code).
