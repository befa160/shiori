-- Lecture (phase 3, lot 3.3) : progression par tome, sens de lecture et disposition.

-- Une ligne par tome ouvert. Clé = identité du tome (hash du fichier, principe 8) : renommer
-- ou déplacer le fichier ne perd rien ; un contenu différent est un autre tome.
CREATE TABLE reading_progress (
  volume_id   TEXT PRIMARY KEY REFERENCES volumes(id) ON DELETE CASCADE,
  page_id     TEXT NOT NULL,     -- dernière page vue : identité stable, jamais un index brut
  ordinal     INTEGER NOT NULL,  -- sa position, pour « 42 / 183 »
  finished_at TEXT,              -- première fois que la dernière page a été atteinte
  updated_at  TEXT NOT NULL
);
CREATE INDEX reading_progress_updated ON reading_progress(updated_at);

-- Le type de bibliothèque donne le sens de lecture par défaut de ses séries (Q12).
ALTER TABLE libraries ADD COLUMN kind TEXT NOT NULL DEFAULT 'manga'
  CHECK (kind IN ('manga','comics','webtoon'));
-- Réglages d'une série ; NULL = défaut de sa bibliothèque.
ALTER TABLE series ADD COLUMN reading_mode TEXT CHECK (reading_mode IN ('rtl','ltr','vertical'));
ALTER TABLE series ADD COLUMN page_layout  TEXT CHECK (page_layout IN ('single','double','auto'));
