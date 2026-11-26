-- Nouveautés suivies (1.3a). Un tome est daté à sa première entrée en base : déplacé ou renommé,
-- c'est le même fichier (même hash), donc pas un nouveau tome. Les tomes déjà en base restent
-- sans date et ne sont jamais nouveaux (Q77).
ALTER TABLE volumes ADD COLUMN added_at TEXT;
CREATE INDEX volumes_added ON volumes(added_at) WHERE added_at IS NOT NULL;

-- Séries suivies par un compte : un tome ajouté après `seen_at` et jamais ouvert est nouveau (Q72).
CREATE TABLE series_follows (
  user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  seen_at   TEXT NOT NULL,
  PRIMARY KEY (user_id, series_id)
);
CREATE INDEX series_follows_series ON series_follows(series_id);

-- Tomes datés déjà ouverts par un compte dont la progression a été effacée (« marquer comme non
-- lu ») : ouverts une fois, ils ne redeviennent pas nouveaux.
CREATE TABLE volumes_opened (
  user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  PRIMARY KEY (user_id, volume_id)
) WITHOUT ROWID;
