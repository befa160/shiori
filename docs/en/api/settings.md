# Settings

Instance settings: account defaults and server behavior.

### `GET /api/settings`

Readable by any signed-in account.

```json
{
  "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null,
  "metadata_adult": false, "theme": "system", "full_scan_hours": 24
}
```

| Field | Values | |
|---|---|---|
| `language` | `fr`, `en`, `null` | account default; `null`: browser language |
| `page_turn` | `reading`, `ltr`, `rtl` | default page-turn direction |
| `theme` | `system`, `dark`, `light`, `oled`, `sepia` | default theme |
| `metadata_online` | boolean | web sources allowed (off by default) |
| `metadata_sources` | `["anilist", "mangadex", "wikipedia"]`, `null` | order of sources; `null`: from the language |
| `metadata_adult` | boolean | adult entries allowed in searches |
| `full_scan_hours` | 0, 1, 6, 24 | full scan of all libraries every N hours; 0: never |

### `PUT /api/settings` <Badge text="admin" />

Partial update: a missing field is unchanged; only `language` and `metadata_sources` accept `null`. An empty body is refused.

```json
{ "metadata_online": true, "full_scan_hours": 6 }
```

**Response 200**: the settings. **Errors**: 400 `invalid_body` (value outside the list, forbidden `null`, duplicate source, empty body).
