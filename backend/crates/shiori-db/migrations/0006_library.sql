-- Bibliothèque façon Plex (phase 4, lot 4.3).

-- Statut de publication, réglé à la main en phase 4 et rempli par les métadonnées en phase 5
-- (Q23). NULL = inconnu.
ALTER TABLE series ADD COLUMN publication_status TEXT
  CHECK (publication_status IN ('ongoing','completed','hiatus','cancelled'));

-- Tri « Récemment ajoutés » (Q27). Le scan l'écrit à la création de la série ; les séries
-- existantes prennent la date de création de leur bibliothèque, faute de mieux.
ALTER TABLE series ADD COLUMN created_at TEXT;
UPDATE series SET created_at = (SELECT created_at FROM libraries WHERE libraries.id = series.library_id);
