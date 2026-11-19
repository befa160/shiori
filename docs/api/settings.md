# Réglages

Réglages de l'instance : défauts des comptes et comportement du serveur.

### `GET /api/settings`

Lisible par tout compte connecté.

```json
{
  "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null,
  "metadata_adult": false, "theme": "system", "full_scan_hours": 24
}
```

| Champ | Valeurs | |
|---|---|---|
| `language` | `fr`, `en`, `null` | défaut des comptes ; `null` : langue du navigateur |
| `page_turn` | `reading`, `ltr`, `rtl` | sens pour tourner les pages par défaut |
| `theme` | `system`, `dark`, `light`, `oled`, `sepia` | thème par défaut |
| `metadata_online` | booléen | sources web autorisées (désactivées par défaut) |
| `metadata_sources` | `["anilist", "mangadex"]`, `null` | ordre des sources ; `null` : d'après la langue |
| `metadata_adult` | booléen | fiches pour adultes permises dans les recherches |
| `full_scan_hours` | 0, 1, 6, 24 | scan complet de toutes les bibliothèques toutes les N heures ; 0 : jamais |

### `PUT /api/settings` <Badge text="admin" />

Mise à jour partielle : un champ absent est inchangé ; seuls `language` et `metadata_sources` acceptent `null`. Un corps vide est refusé.

```json
{ "metadata_online": true, "full_scan_hours": 6 }
```

**Réponse 200** : les réglages. **Erreurs** : 400 `invalid_body` (valeur hors liste, `null` interdit, source en double, corps vide).
