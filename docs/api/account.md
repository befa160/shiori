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
{ "name": "Liseuse", "expires_in_days": 90 }
```

`expires_in_days` : facultatif (sans expiration si absent). **Réponse 201** : le jeton, avec son secret en clair, rendu **une seule fois**.

```json
{ "id": 3, "name": "Liseuse", "prefix": "shiori_t_8f2a1c3d", "created_at": "…", "last_used_at": null, "expires_at": "…", "token": "shiori_t_8f2a1c3d…" }
```

**Erreurs** : 400 `invalid_token_name` (1 à 64 caractères).

### `DELETE /api/me/tokens/{id}`

Révoque un jeton. **Réponse 204**.
