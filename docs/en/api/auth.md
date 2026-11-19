# Authentication

Public routes (no session). See also the [conventions](/en/api/).

### `GET /api/auth/state`

State of the instance and of the request's session.

**Response 200**

```json
{ "setup_required": false, "user": { "id": 1, "username": "fabrice", "role": "admin", "must_change_password": false, "disabled": false, "created_at": "2026-11-12T10:00:00.000Z" } }
```

`setup_required`: no account exists (setup page). `user`: `null` without a valid session.

### `POST /api/auth/setup`

Creates the first administrator, while no account exists.

| Field | Type | |
|---|---|---|
| `token` | string | setup token printed at startup |
| `username` | string | 32 characters at most: letters, digits, `.`, `_`, `-` |
| `password` | string | 8 characters at least |
| `client` | `"browser"` \| `"app"` | optional: `app` returns the session token in the response instead of a cookie |

**Response 200**: like `login`. The account takes over the progress and settings saved before accounts existed.

**Errors**: 403 `invalid_setup_token`, 409 `already_set_up`, 400 `invalid_username`, `weak_password`.

### `POST /api/auth/login`

```json
{ "username": "fabrice", "password": "…", "client": "browser" }
```

**Response 200**, without two-factor: the session (`shiori_session` cookie, or `token` with `client: "app"`).

```json
{ "user": { "id": 1, "username": "fabrice", "role": "admin", "must_change_password": false, "disabled": false, "created_at": "…" } }
```

**Response 200**, with two-factor: a challenge valid for 5 minutes, to present to `POST /api/auth/totp`.

```json
{ "totp_required": true, "challenge": "…" }
```

**Errors**: 401 `invalid_credentials` (unknown name, wrong password or disabled account, without distinction), 429 `rate_limited` with `Retry-After` after several failures (per name and per address).

### `POST /api/auth/totp`

Second factor after `login`.

```json
{ "challenge": "…", "code": "123456" }
```

`code`: the app's 6-digit code, or a recovery code. **Response 200**: the session, like `login`.

**Errors**: 401 `invalid_code`, 401 `challenge_expired`, 429 `rate_limited`.

### `POST /api/auth/logout`

Closes the request's session. **Response 204**, cookie cleared.
