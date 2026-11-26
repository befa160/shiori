-- « À lire » (1.3c) : séries mises de côté par un compte. Une série en sort dès qu'une de ses
-- pages est lue (Q74), dans la transaction de la progression.
CREATE TABLE to_read (
  user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  added_at  TEXT NOT NULL,
  PRIMARY KEY (user_id, series_id)
) WITHOUT ROWID;
CREATE INDEX to_read_series ON to_read(series_id);
