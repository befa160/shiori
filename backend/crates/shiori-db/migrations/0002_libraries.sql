-- Bibliothèques, séries et rattachement des emplacements (phase 2, lot 2.2).

CREATE TABLE libraries (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,
  root       TEXT NOT NULL UNIQUE,        -- chemin absolu, séparateurs '/', sans '/' final
  created_at TEXT NOT NULL
);

CREATE TABLE series (
  id         INTEGER PRIMARY KEY,
  library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  folder     TEXT,                        -- premier dossier sous la racine ; NULL = fichiers posés à la racine
  name       TEXT NOT NULL                -- nom affiché (parseur), recalculé après chaque passe
);
-- Une série par dossier ; les fichiers de la racine sont regroupés par nom.
CREATE UNIQUE INDEX series_folder ON series(library_id, folder) WHERE folder IS NOT NULL;
CREATE UNIQUE INDEX series_root_name ON series(library_id, name) WHERE folder IS NULL;

-- Ce qui dépend du chemin vit sur l'emplacement, pas sur le tome (un tome peut être vu à plusieurs endroits).
-- Supprimer une bibliothèque supprime ses emplacements, jamais les tomes ni les pages.
ALTER TABLE file_locations ADD COLUMN library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE;
ALTER TABLE file_locations ADD COLUMN series_id  INTEGER REFERENCES series(id) ON DELETE SET NULL;
ALTER TABLE file_locations ADD COLUMN kind       TEXT CHECK (kind IN ('volume','chapter','special','unknown'));
ALTER TABLE file_locations ADD COLUMN number     REAL;
ALTER TABLE file_locations ADD COLUMN number_end REAL;
CREATE INDEX file_locations_library ON file_locations(library_id);
CREATE INDEX file_locations_series ON file_locations(series_id);
