-- Progression, préférences et réglages de lecture par compte (phase 7, lot 7.4a, Q48).
-- Ce qui existait avant les comptes va dans des tables `legacy_*`, reprises par le premier
-- administrateur à sa création (Q46) ; s'il existe déjà (créé par `shiori user add`), il les
-- reprend tout de suite.

-- Progression : clé (compte, tome).
ALTER TABLE reading_progress RENAME TO legacy_progress;
DROP INDEX reading_progress_updated;
CREATE TABLE reading_progress (
  user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  volume_id   TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  page_id     TEXT NOT NULL,     -- dernière page vue : identité stable, jamais un index brut
  ordinal     INTEGER NOT NULL,  -- sa position, pour « 42 / 183 »
  finished_at TEXT,              -- première fois que la dernière page a été atteinte
  updated_at  TEXT NOT NULL,
  PRIMARY KEY (user_id, volume_id)
);
CREATE INDEX reading_progress_volume ON reading_progress(volume_id);

-- Réglages de lecture d'une série pour un compte ; NULL = défaut de la bibliothèque.
CREATE TABLE user_series_settings (
  user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  series_id    INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  reading_mode TEXT CHECK (reading_mode IN ('rtl','ltr','vertical')),
  page_layout  TEXT CHECK (page_layout IN ('single','double','auto')),
  page_turn    TEXT CHECK (page_turn IN ('reading','ltr','rtl')),
  PRIMARY KEY (user_id, series_id)
);
CREATE INDEX user_series_settings_series ON user_series_settings(series_id);
CREATE TABLE legacy_series_settings (
  series_id    INTEGER PRIMARY KEY REFERENCES series(id) ON DELETE CASCADE,
  reading_mode TEXT,
  page_layout  TEXT,
  page_turn    TEXT
);
INSERT INTO legacy_series_settings (series_id, reading_mode, page_layout, page_turn)
  SELECT id, reading_mode, page_layout, page_turn FROM series
  WHERE reading_mode IS NOT NULL OR page_layout IS NOT NULL OR page_turn IS NOT NULL;
ALTER TABLE series DROP COLUMN reading_mode;
ALTER TABLE series DROP COLUMN page_layout;
ALTER TABLE series DROP COLUMN page_turn;

-- Préférences d'un compte ; NULL = défaut de l'instance (`app_settings`).
CREATE TABLE user_settings (
  user_id   INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  language  TEXT CHECK (language IN ('fr','en')),
  theme     TEXT CHECK (theme IN ('system','dark','light','oled','sepia')),
  page_turn TEXT CHECK (page_turn IN ('reading','ltr','rtl'))
);

-- Vue fusionnée d'une série en anglais ; `merged` reste celle en français (Q34 : une version
-- par langue de l'interface, chaque compte lit la sienne).
ALTER TABLE series_metadata ADD COLUMN merged_en TEXT NOT NULL DEFAULT '{}';

-- Administrateur déjà créé par la CLI : il reprend tout de suite l'existant.
INSERT INTO reading_progress (user_id, volume_id, page_id, ordinal, finished_at, updated_at)
  SELECT u.id, p.volume_id, p.page_id, p.ordinal, p.finished_at, p.updated_at
  FROM legacy_progress p
  JOIN (SELECT MIN(id) AS id FROM users WHERE role = 'admin') u ON u.id IS NOT NULL;
INSERT INTO user_series_settings (user_id, series_id, reading_mode, page_layout, page_turn)
  SELECT u.id, l.series_id, l.reading_mode, l.page_layout, l.page_turn
  FROM legacy_series_settings l
  JOIN (SELECT MIN(id) AS id FROM users WHERE role = 'admin') u ON u.id IS NOT NULL;
DELETE FROM legacy_progress WHERE EXISTS (SELECT 1 FROM users WHERE role = 'admin');
DELETE FROM legacy_series_settings WHERE EXISTS (SELECT 1 FROM users WHERE role = 'admin');
