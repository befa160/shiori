-- Liens d'invitation (version 1.4, lot 1.4a, Q78) : l'administrateur prévoit le rôle et les
-- bibliothèques du futur compte ; la personne choisit son nom et son mot de passe. Jeton de
-- 256 bits, seul son hash BLAKE3 est gardé ; usage unique, 1, 7 ou 30 jours.
CREATE TABLE invites (
  id          INTEGER PRIMARY KEY,
  token_hash  TEXT NOT NULL UNIQUE,
  role        TEXT NOT NULL CHECK (role IN ('admin', 'reader')),
  created_by  INTEGER REFERENCES users(id) ON DELETE SET NULL,
  created_at  TEXT NOT NULL,
  expires_at  TEXT NOT NULL,
  used_at     TEXT,
  used_by     INTEGER REFERENCES users(id) ON DELETE SET NULL
);

-- Bibliothèques ouvertes au futur lecteur (aucune ligne pour un administrateur).
CREATE TABLE invite_libraries (
  invite_id  INTEGER NOT NULL REFERENCES invites(id) ON DELETE CASCADE,
  library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
  PRIMARY KEY (invite_id, library_id)
);
