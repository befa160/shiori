# Bibliothèques

Une bibliothèque :

```json
{ "id": 1, "name": "Mangas", "root": "/bibliotheques/mangas", "kind": "manga", "watch": true, "series_count": 42, "volume_count": 318 }
```

`kind` : `manga`, `comics` ou `webtoon` (sens de lecture par défaut). `root` : chemin absolu, séparateurs `/`, sans `/` final. `watch` : dossiers surveillés.

### `GET /api/libraries`

Bibliothèques visibles par le compte (toutes pour un administrateur), triées par nom.

### `POST /api/libraries` <Badge text="admin" />

```json
{ "name": "Mangas", "root": "/bibliotheques/mangas", "kind": "manga", "share": false }
```

`kind` : `manga` par défaut. `share` : l'ouvre aux lecteurs existants (sinon visible des administrateurs seuls). La surveillance est active. Le scan n'est pas lancé : appelez [`POST /api/admin/scans`](/api/scans#post-api-admin-scans).

**Réponse 201**

```json
{ "id": 1, "name": "Mangas", "root": "/bibliotheques/mangas", "kind": "manga" }
```

**Erreurs** : 400 `invalid_name` (nom vide), 400 `invalid_root` (pas un chemin absolu vers un dossier accessible), 409 `name_taken`, 409 `root_overlaps` (dossier qui contient une autre bibliothèque, ou contenu dans une).

### `PATCH /api/libraries/{id}` <Badge text="admin" />

Active ou arrête la surveillance des dossiers.

```json
{ "watch": false }
```

**Réponse 204**. **Erreurs** : 400 `invalid_body`, 404 `not_found`.

### `DELETE /api/libraries/{id}` <Badge text="admin" />

Supprime la bibliothèque, ses séries et ses emplacements. Les fichiers ne sont jamais touchés ; les tomes et leur progression restent connus (une bibliothèque recréée sur le même dossier les retrouve). **Réponse 204**.

### `GET /api/libraries/{id}/series`

Séries d'une bibliothèque, triées par nom (tri naturel) ; comme [`GET /api/series`](/api/series#get-api-series).
