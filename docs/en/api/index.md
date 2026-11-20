# API reference

Shiori exposes a JSON API under `/api`. The web app uses nothing else: whatever it does, another program can do.

## Authentication

Every route requires a session, except `GET /api/health` and `/api/auth/*`. Three ways to authenticate:

| Means | Use |
|---|---|
| `shiori_session` cookie | browser, set by `POST /api/auth/login` |
| `Authorization: Bearer <session token>` | native app (`"client": "app"` when signing in) |
| `Authorization: Bearer shiori_t_…` | [API token](/en/api/account#api-tokens): reading and progress only |

An API token only opens `GET` routes outside `/api/me` and `/api/admin`, and `PUT /api/volumes/{id}/progress`; elsewhere, 403 `token_forbidden`. With a temporary password, only `/api/me` and `/api/me/password` answer (403 `password_change_required`).

```bash
curl -H "Authorization: Bearer shiori_t_…" https://shiori.example/api/series
```

## Roles

Routes marked **admin**, and all those under `/api/admin`, answer 403 `forbidden` to a reader. A reader only sees the libraries open to them: the rest answers 404, without saying whether the resource exists. Progress, reading settings and the language of titles are those of the signed-in account.

## Errors

Every error has the same shape, with a **stable code** (that the interface translates) and a readable message (in French):

```json
{ "code": "scan_already_running", "message": "un scan est déjà en cours" }
```

| Status | Common codes |
|---|---|
| 400 | `invalid_body`, `invalid_query`, `invalid_id` |
| 401 | `unauthenticated` |
| 403 | `forbidden`, `token_forbidden`, `password_change_required`, `cross_site`, `invalid_host` |
| 404 | `not_found` |
| 405 | `method_not_allowed` |
| 409 | route-specific conflicts (`scan_already_running`, `name_taken`…) |
| 429 | `rate_limited` (`Retry-After` header) |
| 500 | `internal` (details in the server log, never in the response) |

Route-specific codes are listed on each page.

## Conventions

- **JSON** in and out, `Content-Type: application/json`. An unknown field in a body is refused (`invalid_body`) where the route says so.
- **Partial updates** (`PATCH`, `PUT /api/settings`, `PUT /api/me`): a missing field is unchanged, `null` goes back to the default where allowed.
- **Identifiers**: numbers for libraries, series, accounts and passes; hexadecimal strings for volumes (BLAKE3, 64 characters) and pages (128 bits, 32 characters). They never change as long as the content does not.
- **Dates**: RFC 3339 in UTC (`2026-11-18T21:57:23.374Z`).
- **Cursor pagination**: `limit` (50 by default, 500 at most) and `before` / `after` depending on the list.
- **Writes** refused if the browser announces a request from another site (`Sec-Fetch-Site: cross-site`).

## Server health

### `GET /api/health`

No authentication.

```json
{ "status": "ok", "version": "0.5.0" }
```

## Reference pages

| Page | Routes |
|---|---|
| [Authentication](/en/api/auth) | setup, sign-in, two-factor, sign-out |
| [Account](/en/api/account) | signed-in account, preferences, password, devices, two-factor, API tokens |
| [Accounts (admin)](/en/api/users) | account management |
| [Libraries](/en/api/libraries) | list, creation, watching, deletion |
| [Series](/en/api/series) | lists, detail, editing, reading settings, "Continue reading" |
| [Volumes and pages](/en/api/volumes) | detail, progress, pages, images, covers |
| [Metadata](/en/api/metadata) | web sources, search, links, identification |
| [Scans](/en/api/scans) | start, follow, cancel, history, logs |
| [Health](/en/api/health) | library state, files to repair |
| [Settings](/en/api/settings) | instance settings |
| [Backups](/en/api/backups) | storage, backups, restore |
