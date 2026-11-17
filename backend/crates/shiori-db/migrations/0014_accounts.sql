-- Comptes, sessions et accès par bibliothèque (phase 7, lot 7.2). La progression et les
-- réglages par compte arrivent avec le serveur qui connaît le compte (lot 7.4).

CREATE TABLE users (
  id                    INTEGER PRIMARY KEY,
  username              TEXT NOT NULL,              -- tel que saisi, pour l'affichage
  username_key          TEXT NOT NULL UNIQUE,       -- en minuscules (Unicode) : unicité sans casse
  password_hash         TEXT NOT NULL,              -- chaîne PHC Argon2id
  role                  TEXT NOT NULL CHECK (role IN ('admin', 'reader')),
  must_change_password  INTEGER NOT NULL DEFAULT 1, -- mot de passe provisoire (Q45)
  disabled_at           TEXT,
  created_at            TEXT NOT NULL,
  password_changed_at   TEXT NOT NULL
);

-- Accès explicites d'un lecteur (Q49) ; un administrateur voit tout sans ligne ici.
CREATE TABLE user_libraries (
  user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  PRIMARY KEY (user_id, library_id)
);
CREATE INDEX user_libraries_library ON user_libraries(library_id);

-- Seul le hash du jeton est gardé : une copie de la base ne donne aucune session.
CREATE TABLE sessions (
  id           INTEGER PRIMARY KEY,                 -- pour la liste des appareils
  token_hash   TEXT NOT NULL UNIQUE,                -- BLAKE3 hex du jeton
  user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  label        TEXT NOT NULL,                       -- navigateur et système
  created_at   TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  expires_at   TEXT NOT NULL                        -- glissante, 90 jours (Q47)
);
CREATE INDEX sessions_user ON sessions(user_id);
