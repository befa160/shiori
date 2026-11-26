# Séries

## Listes

### `GET /api/series`

Toutes les séries visibles, triées par nom (tri naturel). `?library_id=` restreint à une bibliothèque. Filtres et tri se font côté client.

```json
[{
  "id": 7, "library_id": 1, "name": "One Piece",
  "alt_titles": ["ワンピース"], "authors": ["Eiichiro Oda"], "tags": ["Aventure", "Shōnen"],
  "volume_count": 3, "read_count": 1, "page_count": 612, "pages_read": 250,
  "reading_status": "in_progress", "publication_status": "ongoing",
  "created_at": "…", "last_read_at": "…",
  "cover_page_id": "3f9c…", "cover_image_id": null, "resume_volume_id": "a41b…"
}]
```

| Champ | |
|---|---|
| `name` | titre affiché : édité, sinon fichiers, sinon sources web, sinon nom du dossier |
| `reading_status` | `unread`, `in_progress`, `read`, d'après la progression du compte |
| `publication_status` | `ongoing`, `completed`, `hiatus`, `cancelled`, ou `null` |
| `cover_page_id` / `cover_image_id` | page de couverture ([image](/api/volumes#get-api-pages-id-image)), ou couverture d'une source ([`/api/covers/{id}`](/api/volumes#get-api-covers-id)), qui l'emporte |
| `resume_volume_id` | tome qu'ouvre « Lire » : la suite, sinon le premier non lu, sinon le premier |

### `GET /api/continue`

« Continuer la lecture » : une entrée par série commencée, la plus récente d'abord ; le tome en cours, ou le suivant quand un tome est fini. `?limit=` : 20 par défaut, 100 au plus.

```json
[{ "series_id": 7, "series_name": "One Piece", "library_id": 1, "volume": { "volume_id": "a41b…", "kind": "volume", "number": 2, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T02.cbz", "progress": { "page_id": "…", "ordinal": 45, "page_count": 204, "finished": false, "updated_at": "…" } }, "updated_at": "…" }]
```

### `GET /api/me/new`

Nouveautés des séries suivies par le compte : les tomes ajoutés depuis le suivi ou le dernier « Tout marquer comme vu », jamais ouverts par ce compte. Un fichier déplacé ou renommé n'est pas nouveau (même tome), un fichier modifié l'est (nouveau tome) ; les tomes déjà là avant la 1.3 ne le sont jamais. Ouvert une fois, un tome ne redevient pas nouveau, même si sa progression est effacée (« Marquer comme non lu »). Les séries vont de la plus récemment complétée à la plus ancienne, chacune dans l'ordre de ses tomes. Session seulement, comme tout `/api/me`.

```json
[{ "series_id": 7, "series_name": "One Piece", "library_id": 1, "volume": { "volume_id": "c07e…", "kind": "volume", "number": 108, "…": "…" }, "added_at": "2026-11-26T09:12:44.512Z" }]
```

### `POST /api/me/new/seen`

« Tout marquer comme vu » : plus aucun tome déjà là n'est nouveau pour le compte. **Réponse 204.**

## Une série

### `GET /api/series/{id}`

```json
{
  "id": 7, "library_id": 1, "name": "One Piece", "folder": "One Piece",
  "metadata": { "title": "One Piece", "alt_titles": [], "summary": "…", "authors": [{ "name": "Eiichiro Oda", "story": true, "art": true }],
    "genres": ["Aventure"], "tags": [], "year": 1997, "publisher": "Glénat", "age_rating": null,
    "publication_status": "ongoing", "links": [], "cover_page_id": null, "cover_image_id": null,
    "main_source": null, "right_to_left": true },
  "locked": ["summary"],
  "sources": [],
  "publication_status": "ongoing",
  "settings": { "reading_mode": "rtl", "page_layout": "auto", "page_turn": "reading", "reading_mode_override": null, "page_layout_override": null, "page_turn_override": null },
  "followed": false, "resume_volume_id": "a41b…", "page_count": 612, "pages_read": 250,
  "volumes": [{ "volume_id": "a41b…", "kind": "volume", "number": 1, "number_end": null, "status": "ok", "page_count": 204, "cover_page_id": "…", "file_name": "One Piece T01.cbz", "title": "Romance Dawn", "progress": { "…": "…" } }]
}
```

- `metadata` : vue fusionnée, dans la langue du compte ([métadonnées](/guide/metadata)).
- `locked` : champs édités, que les sources ne remplacent plus.
- `sources` : état des sources web ([métadonnées](/api/metadata)).
- `settings` : réglages de lecture appliqués pour le compte ; `*_override` : ce que la série impose (`null` : défaut de la bibliothèque, ou défaut global pour `page_turn`).
- `followed` : série suivie par le compte ([nouveautés](#get-api-me-new)).
- `volumes` : dans l'ordre de lecture ; `kind` : `volume`, `chapter`, `special`, `unknown` ; `progress` absent si le tome n'a jamais été ouvert.

### `PATCH /api/series/{id}` <Badge text="admin" />

Modifie les métadonnées. Chaque champ donné est **édité et verrouillé** (`null` compris) ; `unlock` rend des champs aux sources.

```json
{ "summary": "Le résumé de la série…", "genres": ["Aventure", "Comédie"], "unlock": ["title"] }
```

Champs : `title`, `alt_titles`, `summary`, `authors`, `genres`, `tags`, `year`, `publisher`, `age_rating`, `publication_status`, `links`, `cover_page_id` (une page de la série), `cover_image_id`, `main_source`. **Réponse 200** : le détail de la série. **Erreurs** : 400 `invalid_body` (champ inconnu, valeur invalide, page d'une autre série).

### `PATCH /api/series/{id}/settings`

Réglages de lecture de la série pour le compte ; `null` revient au défaut.

```json
{ "reading_mode": "vertical", "page_layout": null, "page_turn": "ltr" }
```

`reading_mode` : `rtl`, `ltr`, `vertical` ; `page_layout` : `single`, `double`, `auto` ; `page_turn` : `reading`, `ltr`, `rtl`. **Réponse 200** : les réglages appliqués.

### `PUT /api/series/{id}/read`

« Marquer la série comme lue », pour le compte : chaque tome lisible passe à sa dernière page (un tome déjà lu garde la date de sa première lecture). **Réponse 200** : le détail de la série. **Erreur** : 404 `not_found`.

### `DELETE /api/series/{id}/read`

« Marquer la série comme non lue », pour le compte : la progression de tous ses tomes est effacée. **Réponse 200** : le détail de la série. **Erreur** : 404 `not_found`.

### `PUT /api/series/{id}/follow`

Suit la série pour le compte : ses tomes ajoutés ensuite apparaissent dans les [nouveautés](#get-api-me-new) ; ceux déjà là, non. Suivre deux fois ne change rien. **Réponse 204.** **Erreur** : 404 `not_found`.

### `DELETE /api/series/{id}/follow`

Ne suit plus la série. **Réponse 204.** **Erreur** : 404 `not_found`.
