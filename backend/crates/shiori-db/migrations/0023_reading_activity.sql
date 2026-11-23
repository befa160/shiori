-- Statistiques de lecture (1.1a) : activité par compte et par jour, jour pris dans le fuseau de
-- l'appareil (Q59). Remplie à chaque progression enregistrée ; « marquer comme lu » n'y entre pas.
CREATE TABLE reading_activity (
  user_id  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  day      TEXT NOT NULL,               -- AAAA-MM-JJ
  pages    INTEGER NOT NULL DEFAULT 0,
  seconds  INTEGER NOT NULL DEFAULT 0,
  finished INTEGER NOT NULL DEFAULT 0,  -- tomes dont la dernière page a été atteinte ce jour-là
  PRIMARY KEY (user_id, day)
) WITHOUT ROWID;

-- Avant cette version, seule la fin des tomes est connue : elle compte le jour où elle a eu
-- lieu (UTC), sans pages ni temps.
INSERT INTO reading_activity (user_id, day, finished)
  SELECT user_id, substr(finished_at, 1, 10), count(*)
  FROM reading_progress
  WHERE finished_at IS NOT NULL
  GROUP BY user_id, substr(finished_at, 1, 10);
