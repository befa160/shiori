-- Fiches pour adultes dans les recherches web (lot 5.7b) : exclues par défaut, la case de
-- l'administration les permet.
ALTER TABLE app_settings ADD COLUMN metadata_adult INTEGER NOT NULL DEFAULT 0;
