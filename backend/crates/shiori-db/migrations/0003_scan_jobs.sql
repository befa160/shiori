-- Passes de scan pilotées (phase 2, lot 2.6) : statut, compteurs et journal par passe.

ALTER TABLE scan_runs ADD COLUMN library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL;
ALTER TABLE scan_runs ADD COLUMN origin     TEXT NOT NULL DEFAULT 'cli' CHECK (origin IN ('cli','api'));
ALTER TABLE scan_runs ADD COLUMN status     TEXT NOT NULL DEFAULT 'completed'
  CHECK (status IN ('running','completed','cancelled','failed','interrupted'));
-- Fichiers par statut : tous les fichiers vus, inchangés compris ; `files_unchanged` en est un sous-ensemble.
ALTER TABLE scan_runs ADD COLUMN files_total     INTEGER;
ALTER TABLE scan_runs ADD COLUMN files_ok        INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_warning   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_error     INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_unchanged INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_runs ADD COLUMN files_removed   INTEGER NOT NULL DEFAULT 0;  -- emplacements disparus
ALTER TABLE scan_runs ADD COLUMN error           TEXT;                        -- erreur fatale
-- Le défaut ne sert qu'aux lignes existantes : le code écrit toujours `status`.
UPDATE scan_runs SET status = 'interrupted' WHERE finished_at IS NULL;      -- passes de phase 1 coupées
CREATE INDEX scan_runs_started ON scan_runs(started_at);

CREATE TABLE scan_run_logs (
  id          INTEGER PRIMARY KEY,
  scan_run_id INTEGER NOT NULL REFERENCES scan_runs(id) ON DELETE CASCADE,
  at          TEXT NOT NULL,
  level       TEXT NOT NULL CHECK (level IN ('info','warning','error')),
  path        TEXT,
  code        TEXT,                        -- code de diagnostic quand il y en a un
  message     TEXT NOT NULL
);
CREATE INDEX scan_run_logs_run ON scan_run_logs(scan_run_id, id);
