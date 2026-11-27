-- Collections partagées (1.3d) : un nom et des séries, choisis par l'administrateur, visibles de
-- tous les comptes qui voient leurs séries (Q76).
CREATE TABLE collections (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created_at TEXT NOT NULL
);
CREATE TABLE collection_series (
  collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  series_id     INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  PRIMARY KEY (collection_id, series_id)
) WITHOUT ROWID;
CREATE INDEX collection_series_series ON collection_series(series_id);
