# Comptes (admin)

Gestion des comptes, réservée aux administrateurs.

Un compte vu par l'administrateur :

```json
{ "id": 2, "username": "lea", "role": "reader", "must_change_password": true, "disabled": false, "created_at": "…", "libraries": [1, 3], "totp": false }
```

`libraries` : bibliothèques ouvertes à un lecteur (vide pour un administrateur, qui les voit toutes).

### `GET /api/admin/users`

Tous les comptes.

### `POST /api/admin/users`

```json
{ "username": "lea", "role": "reader", "libraries": [1, 3] }
```

`role` : `reader` par défaut. `libraries` : facultatif, toutes les bibliothèques existantes si absent. **Réponse 201** : le compte et son mot de passe provisoire, montré une seule fois.

```json
{ "user": { "id": 2, "username": "lea", "…": "…" }, "password": "tW7k-p2Qx-9fLm" }
```

**Erreurs** : 400 `invalid_username`, 409 `name_taken`.

### `PATCH /api/admin/users/{id}`

```json
{ "role": "admin", "disabled": false, "libraries": [1] }
```

Chaque champ est facultatif. Désactiver un compte ferme ses sessions. **Réponse 200** : le compte. **Erreurs** : 409 `last_admin` (le dernier administrateur actif ne peut être ni rétrogradé ni désactivé).

### `DELETE /api/admin/users/{id}`

Supprime le compte et sa progression (pas les fichiers ni les fiches). **Réponse 204**. **Erreurs** : 409 `last_admin`.

### `POST /api/admin/users/{id}/password`

Nouveau mot de passe provisoire (mot de passe oublié) ; ferme les sessions du compte.

```json
{ "password": "tW7k-p2Qx-9fLm" }
```

### `DELETE /api/admin/users/{id}/sessions`

Déconnecte tous les appareils du compte et révoque ses jetons d'API. **Réponse 204**.

### `DELETE /api/admin/users/{id}/totp`

Retire la double authentification (téléphone perdu). **Réponse 204**.
