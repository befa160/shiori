-- Lot 9.1 : surveillance des dossiers et scan complet périodique.

-- Chaque bibliothèque est surveillée par défaut ; l'administrateur peut l'arrêter (montage
-- réseau très gros, disque qu'on ne veut pas réveiller).
ALTER TABLE libraries ADD COLUMN watch INTEGER NOT NULL DEFAULT 1;

-- Scan complet de toutes les bibliothèques toutes les N heures, filet de la surveillance
-- (montages réseau, changements faits serveur éteint) ; 0 = jamais.
ALTER TABLE app_settings ADD COLUMN full_scan_hours INTEGER NOT NULL DEFAULT 24
  CHECK (full_scan_hours IN (0, 1, 6, 24));
