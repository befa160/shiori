-- Réglages de l'installation (lot i18n) : une seule ligne tant qu'il n'y a pas de comptes ;
-- avec l'authentification, ils deviendront des réglages par compte.
CREATE TABLE app_settings (
  id       INTEGER PRIMARY KEY CHECK (id = 1),
  language TEXT CHECK (language IN ('fr','en'))   -- NULL : langue du navigateur
);
INSERT INTO app_settings (id) VALUES (1);
