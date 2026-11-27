# Réglages

Réglages de l'instance : défauts des comptes et comportement du serveur.

### `GET /api/settings`

Lisible par tout compte connecté.

```json
{
  "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null,
  "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false
}
```

| Champ | Valeurs | |
|---|---|---|
| `language` | `fr`, `en`, `null` | défaut des comptes ; `null` : langue du navigateur |
| `page_turn` | `reading`, `ltr`, `rtl` | sens pour tourner les pages par défaut |
| `theme` | `system`, `dark`, `light`, `oled`, `sepia` | thème par défaut |
| `metadata_online` | booléen | sources web autorisées (désactivées par défaut) |
| `metadata_sources` | `["anilist", "mangadex", "wikipedia", "mangaupdates", "comicvine"]`, `null` | ordre des sources ; `null` : d'après la langue |
| `metadata_adult` | booléen | fiches pour adultes permises dans les recherches |
| `full_scan_hours` | 0, 1, 6, 24 | scan complet de toutes les bibliothèques toutes les N heures ; 0 : jamais |
| `metadata_refresh_days` | 0, 7, 30 | fiches web liées relues quand elles ont plus de N jours, par tranches de 100 par heure ; 0 : jamais (par défaut) |
| `comicvine_key_set` | booléen | une clé Comic Vine est enregistrée (la clé n'est jamais renvoyée) |

### `PUT /api/settings` <Badge text="admin" />

Mise à jour partielle : un champ absent est inchangé ; seuls `language`, `metadata_sources` et `comicvine_key` acceptent `null`. `comicvine_key` (écriture seulement) : la clé d'API Comic Vine, 1 à 128 lettres ou chiffres, `null` pour l'effacer ; gardée en base (donc dans les sauvegardes), jamais renvoyée. Un corps vide est refusé.

```json
{ "metadata_online": true, "full_scan_hours": 6 }
```

**Réponse 200** : les réglages. **Erreurs** : 400 `invalid_body` (valeur hors liste, `null` interdit, source en double, corps vide).
