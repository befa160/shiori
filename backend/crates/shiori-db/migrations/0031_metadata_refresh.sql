-- Actualisation périodique des fiches web liées (version 1.4, lot 1.4f) : une fiche relue si
-- elle date de plus de N jours ; 0 = jamais (par défaut, Q80).
ALTER TABLE app_settings ADD COLUMN metadata_refresh_days INTEGER NOT NULL DEFAULT 0
  CHECK (metadata_refresh_days IN (0, 7, 30));
