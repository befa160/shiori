# Accounts (admin)

Account management, for administrators only.

An account as seen by the administrator:

```json
{ "id": 2, "username": "lea", "role": "reader", "must_change_password": true, "disabled": false, "created_at": "…", "libraries": [1, 3], "totp": false }
```

`libraries`: libraries open to a reader (empty for an administrator, who sees them all).

### `GET /api/admin/users`

All accounts.

### `POST /api/admin/users`

```json
{ "username": "lea", "role": "reader", "libraries": [1, 3] }
```

`role`: `reader` by default. `libraries`: optional, all existing libraries if missing. **Response 201**: the account and its temporary password, shown only once.

```json
{ "user": { "id": 2, "username": "lea", "…": "…" }, "password": "tW7k-p2Qx-9fLm" }
```

**Errors**: 400 `invalid_username`, 409 `name_taken`.

### `PATCH /api/admin/users/{id}`

```json
{ "role": "admin", "disabled": false, "libraries": [1] }
```

Each field is optional. Disabling an account closes its sessions. **Response 200**: the account. **Errors**: 409 `last_admin` (the last active administrator can be neither demoted nor disabled).

### `DELETE /api/admin/users/{id}`

Deletes the account and its progress (not the files or entries). **Response 204**. **Errors**: 409 `last_admin`.

### `POST /api/admin/users/{id}/password`

New temporary password (forgotten password); closes the account's sessions.

```json
{ "password": "tW7k-p2Qx-9fLm" }
```

### `DELETE /api/admin/users/{id}/sessions`

Signs out all the account's devices and revokes its API tokens. **Response 204**.

### `DELETE /api/admin/users/{id}/totp`

Removes two-factor authentication (lost phone). **Response 204**.

## Invitations

An invite link (`/invite/<token>`) lets someone create their own account, with the role and libraries the administrator chose. Single use, valid for 1, 7 or 30 days; only the token's hash is kept. An invite dies when its author is no longer an administrator (demoted, disabled or deleted). No open sign-up.

An invite as the administrator sees it (never its token):

```json
{ "id": 4, "role": "reader", "libraries": [1, 3], "created_at": "…", "expires_at": "…" }
```

### `GET /api/admin/invites`

Invites not used yet, expired ones included (to revoke), newest first.

### `POST /api/admin/invites`

```json
{ "role": "reader", "libraries": [1, 3], "days": 7 }
```

Every field is optional: `role` `reader`, `days` 7 (1, 7 or 30), `libraries` every existing library (ignored for an administrator). **Response 201**: the invite and its token, shown only once.

```json
{ "id": 4, "role": "reader", "libraries": [1, 3], "created_at": "…", "expires_at": "…", "token": "…" }
```

**Errors**: 400 `invalid_body` (duration not in the list).

### `DELETE /api/admin/invites/{id}`

Revokes an invite not used yet. **Response 204**. **Errors**: 404 `not_found`.

The invited person then uses the link without a session, see [Authentication](./auth.md#get-api-auth-invite-token).
