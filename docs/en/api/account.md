# Account

The signed-in account. These routes are refused to API tokens.

## Account and preferences

### `GET /api/me`

```json
{
  "user": { "id": 2, "username": "lea", "role": "reader", "must_change_password": false, "disabled": false, "created_at": "…" },
  "preferences": { "language": "fr", "theme": null, "page_turn": null },
  "totp": true,
  "recovery_codes_left": 9
}
```

A `null` preference follows the instance default ([settings](/en/api/settings)).

### `PUT /api/me`

Account preferences, partial update: `language` (`fr`, `en`), `theme` (`system`, `dark`, `light`, `oled`, `sepia`), `page_turn` (`reading`, `ltr`, `rtl`); `null` goes back to the instance default. **Response 200**: like `GET /api/me`.

### `POST /api/me/password`

```json
{ "current": "…", "new": "…" }
```

**Response 204**. Closes the sessions of the other devices and lifts the obligation to change a temporary password. **Errors**: 403 `wrong_password`, 400 `weak_password`.

## Devices

### `GET /api/me/sessions`

```json
[{ "id": 12, "label": "Mozilla/5.0 (Linux; Android 14…)", "created_at": "…", "last_seen_at": "…", "current": true }]
```

### `DELETE /api/me/sessions/{id}`

Signs out a device. **Response 204**, 404 if the session does not belong to this account.

## Reading statistics

### `GET /api/me/stats`

Statistics of the session account, never those of another account. Refused to API tokens.

Parameters: `days` (period in days, today included: 84 by default, from 1 to 366), `utc_offset` (device time zone in minutes east of UTC, which sets “today”; UTC by default).

```json
{
  "days": [{ "day": "2026-11-23", "pages": 184, "seconds": 2460, "finished": 1 }],
  "total": { "pages": 5210, "seconds": 70310, "finished": 31 },
  "series_finished": 4
}
```

`days`: days read in the period, oldest first; a day without reading is missing. `pages`: pages moved forward (a jump of more than 50 pages at once does not count, except reading done offline); `seconds`: estimated time, without breaks of 5 minutes or more; `finished`: volumes whose last page was reached. `total`: since the start; before 1.1, only finished volumes are known. `series_finished`: visible series whose readable volumes are all read. **Error**: 400 `invalid_query`.

## Two-factor authentication

### `POST /api/me/totp`

Starts enrollment.

```json
{ "password": "…" }
```

**Response 200**: the secret (to type by hand), the `otpauth://` address and an SVG QR code.

```json
{ "secret": "JBSWY3DPEHPK3PXP…", "uri": "otpauth://totp/Shiori:lea?secret=…&issuer=Shiori", "qr_svg": "<svg …>" }
```

**Errors**: 403 `wrong_password`, 409 `totp_enabled`.

### `POST /api/me/totp/confirm`

```json
{ "code": "123456" }
```

**Response 200**: 10 recovery codes, shown only once.

```json
{ "recovery_codes": ["k3f9-2hq8", "…"] }
```

**Errors**: 400 `invalid_code`, 409 `totp_not_started`.

### `DELETE /api/me/totp`

```json
{ "password": "…", "code": "123456" }
```

**Response 204**. **Errors**: 403 `wrong_password`, 403 `invalid_code`, 409 `totp_disabled`.

## API tokens

A token lets another program read the account's libraries and save its progress, without the password.

### `GET /api/me/tokens`

```json
[{ "id": 3, "name": "E-reader", "prefix": "shiori_t_8f2a1c3d", "created_at": "…", "last_used_at": "…", "expires_at": null }]
```

### `POST /api/me/tokens`

```json
{ "name": "E-reader", "expires_in_days": 90, "password": "…" }
```

`expires_in_days`: optional (no expiry if missing). `password`: the account password, required (a token outlives a password change: a stolen session must not be able to create one). **Response 201**: the token, with its secret in clear, returned **only once**.

```json
{ "id": 3, "name": "E-reader", "prefix": "shiori_t_8f2a1c3d", "created_at": "…", "last_used_at": null, "expires_at": "…", "token": "shiori_t_8f2a1c3d…" }
```

**Errors**: 400 `invalid_token_name` (1 to 64 characters), 403 `wrong_password`, 429 `rate_limited`.

### `DELETE /api/me/tokens/{id}`

Revokes a token. **Response 204**.
