# Santé

Routes **admin** de la page Santé des bibliothèques.

### `GET /api/admin/health`

```json
{
  "libraries": [
    { "id": 1, "name": "Mangas", "ok": 300, "warning": 16, "error": 2, "unreadable": 1, "deep_checked_at": "2026-11-10T08:00:00.000Z" }
  ],
  "codes": [
    { "code": "crc_mismatch", "severity": "error", "files": 2 },
    { "code": "archive_order_differs", "severity": "warning", "files": 12 }
  ]
}
```

- `ok`, `warning`, `error` : tomes distincts par état de leur dernier scan.
- `unreadable` : fichiers sans tome fiable (illisibles avant même leur hash).
- `deep_checked_at` : fin de la dernière passe `deep` terminée sur toute la bibliothèque ; `null` : jamais.
- `codes` : avertissements et erreurs présents, des plus graves aux plus fréquents, avec le nombre de fichiers concernés.

### `GET /api/admin/health/files`

Fichiers à réparer (tomes en avertissement ou en erreur, fichiers illisibles), triés par chemin.

| Paramètre | |
|---|---|
| `library_id` | une bibliothèque |
| `code` | un [code de diagnostic](/guide/diagnostics) |
| `after` | chemin du dernier fichier reçu (pagination) |
| `limit` | 50 par défaut, 500 au plus |

```json
[{
  "path": "/bibliotheques/mangas/One Piece/One Piece T01.cbz", "library_id": 1, "volume_id": "a41b…", "series_id": 7, "status": "error",
  "issues": [{ "severity": "error", "code": "crc_mismatch", "entry_name": "003.png", "message": "…" }]
}]
```

`volume_id` et `series_id` : `null` pour un fichier illisible. Problèmes triés, erreurs d'abord. **Erreurs** : 400 `invalid_query` (code inconnu).
