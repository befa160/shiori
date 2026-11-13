-- Sources web d'une série (lot 5.5b). Une ligne par série et par source, selon l'état :
-- liée (`external_id`, `data` = dernière fiche), cherchée sans lien (`candidates` à confirmer,
-- ou vide : rien trouvé), ou en échec (`last_error`, code stable ; retentée plus tard).
CREATE TABLE series_sources (
  series_id   INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
  source      TEXT NOT NULL CHECK (source IN ('anilist','mangadex')),
  external_id TEXT,
  matched_by  TEXT CHECK (matched_by IN ('auto','user')),
  data        TEXT,                -- shiori_meta::WebMetadata en JSON
  candidates  TEXT,                -- [WebMetadata] en JSON, quand plusieurs fiches conviennent
  checked_at  TEXT NOT NULL,
  last_error  TEXT,
  PRIMARY KEY (series_id, source)
);

-- Réseau désactivé par défaut (Q33) ; ordre des sources choisi, `NULL` = d'après la langue
-- de l'interface (anglais : AniList d'abord, sinon MangaDex d'abord).
ALTER TABLE app_settings ADD COLUMN metadata_online INTEGER NOT NULL DEFAULT 0;
ALTER TABLE app_settings ADD COLUMN metadata_sources TEXT;
