# Tomes et pages

## Tomes

### `GET /api/volumes/{id}`

`id` : identifiant du tome (BLAKE3 hexadécimal du fichier).

```json
{
  "id": "a41b…", "size": 48213504, "format": "cbz", "status": "warning", "page_count": 204, "scanned_at": "…",
  "locations": [{ "path": "/bibliotheques/mangas/One Piece/One Piece T01.cbz", "library_id": 1, "series_id": 7 }],
  "issues": [{ "code": "duplicate_entry_identical", "severity": "warning", "entry": null, "message": "…" }],
  "settings": { "reading_mode": "rtl", "page_layout": "auto", "page_turn": "reading", "reading_mode_override": null, "page_layout_override": null, "page_turn_override": null },
  "progress": null
}
```

`status` : `ok`, `warning`, `error` ; `format` : d'après le contenu, jamais l'extension ; `issues` : problèmes du dernier scan ([codes](/guide/diagnostics)).

### `PATCH /api/volumes/{id}` <Badge text="admin" />

Métadonnées du tome, comme pour une série : champ donné = édité et verrouillé, `unlock` le rend aux fichiers. Champs : `title`, `summary`, `release_date`, `authors`, `cover_page_id` (une page de ce tome, sinon 400 `invalid_body` ; sans valeur, sa première page). **Réponse 200** : le détail du tome.

## Pages

### `GET /api/volumes/{id}/pages`

Les pages dans l'ordre de lecture, établi par le scan. Le lecteur n'en déduit rien d'autre.

```json
[{ "id": "3f9c0a…", "ordinal": 0, "width": 1600, "height": 2400, "media_type": "image/jpeg", "status": "ok" }]
```

`status` : `ok` ou `broken` (page illisible dans l'archive : son image répond une erreur).

### `GET /api/pages/{id}/image`

Une page, par son identifiant (jamais par un index). Sans paramètre : l'originale. Avec `w` : redimensionnée.

| Paramètre | Valeurs |
|---|---|
| `w` | 160, 320, 480, 640, 800, 1080, 1280, 1600, 2048 |
| `f` | `avif` (défaut), `jpeg` ; exige `w` |

La page n'est jamais agrandie (plus étroite que `w` : l'originale) ; une page en AVIF est servie telle quelle. Réponse avec `Cache-Control: immutable` et un `ETag` ; `If-None-Match` donne 304.

**Erreurs** :

| Statut | Code | Cas |
|---|---|---|
| 400 | `invalid_width`, `invalid_format` | paramètre hors liste |
| 404 | `page_not_found` | page inconnue ou pas visible |
| 409 | `volume_changed` | le fichier a changé depuis le scan : à rescanner |
| 409 | `page_mismatch` | l'archive ne contient plus cette page à sa place : à rescanner |
| 410 | `volume_unavailable` | aucun fichier connu de ce tome n'existe encore |
| 422 | `page_broken` | page illisible dans l'archive (voir les problèmes du tome) |
| 500 | `page_corrupt`, `render_failed` | contenu abîmé à la lecture, échec du rendu |

Jamais une autre page à la place : toute incohérence est une erreur explicite.

### `GET /api/covers/{id}`

Couverture d'une source web gardée par le serveur (`cover_image_id`), originale ou `?w=&f=` comme une page.

## Progression

### `GET /api/volumes/{id}/progress`

```json
{ "page_id": "3f9c…", "ordinal": 45, "page_count": 204, "finished": false, "updated_at": "…" }
```

**Réponse 204** si le tome n'a jamais été ouvert par ce compte.

### `PUT /api/volumes/{id}/progress`

Enregistre la page lue. Permis aux jetons d'API.

```json
{ "page_id": "3f9c…", "read_at": "2026-11-19T08:12:00.000Z", "utc_offset": 60 }
```

`read_at` : facultatif, heure de lecture (ISO 8601) d'une page lue hors ligne et envoyée plus tard ; absent : maintenant, et une heure future compte comme maintenant. La page la plus récente l'emporte : une page lue hors ligne ne remplace pas celle lue depuis sur un autre appareil.

`utc_offset` : facultatif, fuseau de l'appareil en minutes à l'est d'UTC (UTC par défaut, borné à ±840), qui donne le jour compté par les [statistiques](account#statistiques-de-lecture). `stats` : facultatif, `false` pour un enregistrement qui n'est pas une lecture (« Marquer comme lu ») et n'entre pas dans les statistiques.

**Réponse 200** : la progression gardée. `finished` devient vrai dès que la dernière page est atteinte, même par une page plus ancienne. **Erreurs** : 400 `invalid_body` (dont `read_at` qui n'est pas une date), 404 `not_found`, 422 `page_not_in_volume`.

### `DELETE /api/volumes/{id}/progress`

« Marquer comme non lu ». **Réponse 204**.
