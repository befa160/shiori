-- Lot 9.3 : date de la dernière vérification complète (passe `deep` terminée sur la racine).
-- Gardée sur la bibliothèque : les passes et leurs journaux sont purgés après 30 jours.
ALTER TABLE libraries ADD COLUMN deep_checked_at TEXT;
