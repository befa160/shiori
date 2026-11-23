# Compte

Le compte connecté. Ces routes sont refusées aux jetons d'API.

## Compte et préférences

### `GET /api/me`

```json
{
  "user": { "id": 2, "username": "lea", "role": "reader", "must_change_password": false, "disabled": false, "created_at": "…" },
  "preferences": { "language": "fr", "theme": null, "page_turn": null },
  "totp": true,
  "recovery_codes_left": 9
}
```

Une préférence `null` suit le défaut de l'instance ([réglages](/api/settings)).

### `PUT /api/me`

Préférences du compte, mise à jour partielle : `language` (`fr`, `en`), `theme` (`system`, `dark`, `light`, `oled`, `sepia`), `page_turn` (`reading`, `ltr`, `rtl`) ; `null` revient au défaut de l'instance. **Réponse 200** : comme `GET /api/me`.

### `POST /api/me/password`

```json
{ "current": "…", "new": "…" }
```

**Réponse 204**. Ferme les sessions des autres appareils et lève l'obligation de changer un mot de passe provisoire. **Erreurs** : 403 `wrong_password`, 400 `weak_password`.

## Appareils

### `GET /api/me/sessions`

```json
[{ "id": 12, "label": "Mozilla/5.0 (Linux; Android 14…)", "created_at": "…", "last_seen_at": "…", "current": true }]
```

### `DELETE /api/me/sessions/{id}`

Déconnecte un appareil. **Réponse 204**, 404 si la session n'est pas à ce compte.

## Statistiques de lecture

### `GET /api/me/stats`

Statistiques du compte de la session, jamais celles d'un autre compte. Refusé aux jetons d'API.

Paramètres : `days` (période en jours, aujourd'hui compris : 84 par défaut, de 1 à 366), `utc_offset` (fuseau de l'appareil en minutes à l'est d'UTC, qui fixe « aujourd'hui » ; UTC par défaut).

```json
{
  "days": [{ "day": "2026-11-23", "pages": 184, "seconds": 2460, "finished": 1 }],
  "total": { "pages": 5210, "seconds": 70310, "finished": 31 },
  "series_finished": 4
}
```

`days` : jours lus de la période, du plus ancien au plus récent ; un jour sans lecture est absent. `pages` : pages avancées (un saut de plus de 50 pages d'un coup ne compte pas, sauf une lecture hors ligne) ; `seconds` : temps estimé, sans les pauses de 5 minutes et plus ; `finished` : tomes dont la dernière page a été atteinte. `total` : depuis le début ; avant la 1.1, seuls les tomes terminés sont connus. `series_finished` : séries visibles dont tous les tomes lisibles sont lus. **Erreur** : 400 `invalid_query`.

## Double authentification

### `POST /api/me/totp`

Commence l'activation.

```json
{ "password": "…" }
```

**Réponse 200** : le secret (à saisir à la main), l'adresse `otpauth://` et un QR code en SVG.

```json
{ "secret": "JBSWY3DPEHPK3PXP…", "uri": "otpauth://totp/Shiori:lea?secret=…&issuer=Shiori", "qr_svg": "<svg …>" }
```

**Erreurs** : 403 `wrong_password`, 409 `totp_enabled`.

### `POST /api/me/totp/confirm`

```json
{ "code": "123456" }
```

**Réponse 200** : 10 codes de secours, montrés une seule fois.

```json
{ "recovery_codes": ["k3f9-2hq8", "…"] }
```

**Erreurs** : 400 `invalid_code`, 409 `totp_not_started`.

### `DELETE /api/me/totp`

```json
{ "password": "…", "code": "123456" }
```

**Réponse 204**. **Erreurs** : 403 `wrong_password`, 403 `invalid_code`, 409 `totp_disabled`.

## Jetons d'API

Un jeton permet à un autre logiciel de lire les bibliothèques du compte et d'enregistrer sa progression, sans le mot de passe.

### `GET /api/me/tokens`

```json
[{ "id": 3, "name": "Liseuse", "prefix": "shiori_t_8f2a1c3d", "created_at": "…", "last_used_at": "…", "expires_at": null }]
```

### `POST /api/me/tokens`

```json
{ "name": "Liseuse", "expires_in_days": 90, "password": "…" }
```

`expires_in_days` : facultatif (sans expiration si absent). `password` : le mot de passe du compte, exigé (un jeton survit au changement de mot de passe : une session volée ne doit pas pouvoir en créer un). **Réponse 201** : le jeton, avec son secret en clair, rendu **une seule fois**.

```json
{ "id": 3, "name": "Liseuse", "prefix": "shiori_t_8f2a1c3d", "created_at": "…", "last_used_at": null, "expires_at": "…", "token": "shiori_t_8f2a1c3d…" }
```

**Erreurs** : 400 `invalid_token_name` (1 à 64 caractères), 403 `wrong_password`, 429 `rate_limited`.

### `DELETE /api/me/tokens/{id}`

Révoque un jeton. **Réponse 204**.
