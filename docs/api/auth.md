# Authentification

Routes publiques (sans session). Voir aussi les [conventions](/api/).

### `GET /api/auth/state`

État de l'instance et de la session de la requête.

**Réponse 200**

```json
{ "setup_required": false, "user": { "id": 1, "username": "fabrice", "role": "admin", "must_change_password": false, "disabled": false, "created_at": "2026-11-12T10:00:00.000Z" } }
```

`setup_required` : aucun compte n'existe (page d'installation). `user` : `null` sans session valide.

### `POST /api/auth/setup`

Crée le premier administrateur, tant qu'aucun compte n'existe.

| Champ | Type | |
|---|---|---|
| `token` | chaîne | jeton d'installation affiché au démarrage |
| `username` | chaîne | 32 caractères au plus : lettres, chiffres, `.`, `_`, `-` |
| `password` | chaîne | 8 caractères au moins |
| `client` | `"browser"` \| `"app"` | facultatif : `app` renvoie le jeton de session dans la réponse au lieu d'un cookie |

**Réponse 200** : comme `login`. Le compte reprend la progression et les réglages enregistrés avant l'arrivée des comptes.

**Erreurs** : 403 `invalid_setup_token`, 409 `already_set_up`, 400 `invalid_username`, `weak_password`.

### `POST /api/auth/login`

```json
{ "username": "fabrice", "password": "…", "client": "browser" }
```

**Réponse 200**, sans double authentification : la session (cookie `shiori_session`, ou `token` avec `client: "app"`).

```json
{ "user": { "id": 1, "username": "fabrice", "role": "admin", "must_change_password": false, "disabled": false, "created_at": "…" } }
```

**Réponse 200**, avec double authentification : un défi valable 5 minutes, à présenter à `POST /api/auth/totp`.

```json
{ "totp_required": true, "challenge": "…" }
```

**Erreurs** : 401 `invalid_credentials` (nom inconnu, mot de passe faux ou compte désactivé, sans distinction), 429 `rate_limited` avec `Retry-After` après plusieurs échecs (par nom et par adresse).

### `POST /api/auth/totp`

Second facteur après `login`.

```json
{ "challenge": "…", "code": "123456" }
```

`code` : le code à 6 chiffres de l'application, ou un code de secours. **Réponse 200** : la session, comme `login`.

**Erreurs** : 401 `invalid_code`, 401 `challenge_expired`, 429 `rate_limited`.

### `POST /api/auth/logout`

Ferme la session de la requête. **Réponse 204**, cookie effacé.

### `GET /api/auth/invite/{token}`

Sans session : le lien d'invitation est-il encore bon ? **Réponse 200** : le rôle prévu, rien d'autre.

```json
{ "role": "reader" }
```

**Erreurs** : 404 `invite_invalid` (lien inconnu, expiré, déjà utilisé ou révoqué, sans distinction).

### `POST /api/auth/invite/{token}`

Sans session : crée le compte de l'invitation (mot de passe définitif, rôle et bibliothèques prévus) et ouvre sa session, comme `login`.

```json
{ "username": "lea", "password": "…" }
```

**Réponse 200** : la session (cookie `shiori_session`, ou `token` avec `client: "app"`). **Erreurs** : 404 `invite_invalid`, 400 `invalid_username`, 400 `weak_password`, 409 `name_taken` (le lien reste valable), 429 `rate_limited` après plusieurs liens invalides depuis la même adresse.
