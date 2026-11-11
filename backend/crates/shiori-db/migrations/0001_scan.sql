-- Résultats du scan d'intégrité : la source de vérité sur les tomes et leurs pages.
-- Les tables des phases suivantes référenceront volumes.id et pages.id, jamais un
-- chemin ou un index brut.

CREATE TABLE volumes (
  id              TEXT PRIMARY KEY,          -- BLAKE3 hex du fichier
  size            INTEGER NOT NULL,
  format          TEXT NOT NULL,             -- 'cbz' (plus tard 'cbr')
  status          TEXT NOT NULL CHECK (status IN ('ok', 'warning', 'error')),
  page_count      INTEGER NOT NULL,          -- nombre réel de pages (= COUNT(pages))
  scanner_version INTEGER NOT NULL,          -- un changement des règles déclenche un rescan
  scanned_at      TEXT NOT NULL              -- RFC 3339 UTC
);

CREATE TABLE file_locations (                -- où le tome a été vu ; plusieurs chemins possibles
  path         TEXT PRIMARY KEY,             -- chemin absolu, séparateurs '/'
  volume_id    TEXT NOT NULL REFERENCES volumes(id),
  size         INTEGER NOT NULL,
  mtime_ns     INTEGER NOT NULL,
  last_seen_at TEXT NOT NULL
);
CREATE INDEX file_locations_volume ON file_locations(volume_id);

CREATE TABLE pages (
  id              TEXT PRIMARY KEY,          -- 128 bits hex
  volume_id       TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  ordinal         INTEGER NOT NULL,
  entry_name      TEXT NOT NULL,             -- nom décodé (affichage, diagnostics)
  entry_name_raw  BLOB NOT NULL,             -- octets bruts (identité)
  archive_index   INTEGER NOT NULL,          -- index d'accès direct dans l'archive
  media_type      TEXT,
  width           INTEGER,
  height          INTEGER,
  compressed_size INTEGER NOT NULL,
  size            INTEGER NOT NULL,
  crc32           INTEGER NOT NULL,
  status          TEXT NOT NULL CHECK (status IN ('ok', 'broken')),
  UNIQUE (volume_id, ordinal)
);

CREATE TABLE scan_runs (
  id          INTEGER PRIMARY KEY,
  root        TEXT NOT NULL,
  options     TEXT NOT NULL,                 -- JSON
  started_at  TEXT NOT NULL,
  finished_at TEXT
);

CREATE TABLE scan_issues (                   -- état courant : problèmes du dernier scan de chaque tome
  id          INTEGER PRIMARY KEY,
  scan_run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
  volume_id   TEXT REFERENCES volumes(id) ON DELETE CASCADE, -- NULL si aucun tome fiable
  path        TEXT NOT NULL,
  severity    TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error')),
  code        TEXT NOT NULL,
  entry_name  TEXT,
  message     TEXT NOT NULL
);
CREATE INDEX scan_issues_volume ON scan_issues(volume_id);
CREATE INDEX scan_issues_path ON scan_issues(path);
