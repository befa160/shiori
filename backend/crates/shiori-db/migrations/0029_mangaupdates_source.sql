-- MangaUpdates, quatrième source web (version 1.4, lot 1.4c). SQLite ne modifie pas une
-- contrainte CHECK : la table est reconstruite à l'identique, la liste des sources élargie.
CREATE TABLE series_sources_new (
  series_id   INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  source      TEXT NOT NULL CHECK (source IN ('anilist','mangadex','wikipedia','mangaupdates')),
  external_id TEXT,
  matched_by  TEXT CHECK (matched_by IN ('auto','user')),
  data        TEXT,
  candidates  TEXT,
  checked_at  TEXT NOT NULL,
  last_error  TEXT,
  unlinked    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (series_id, source)
);
INSERT INTO series_sources_new SELECT series_id, source, external_id, matched_by, data,
  candidates, checked_at, last_error, unlinked FROM series_sources;
DROP TABLE series_sources;
ALTER TABLE series_sources_new RENAME TO series_sources;

-- Un ordre des sources déjà choisi ne la connaît pas : elle s'y ajoute en dernier (Q81).
UPDATE app_settings SET metadata_sources = json_insert(metadata_sources, '$[#]', 'mangaupdates')
WHERE metadata_sources IS NOT NULL;
