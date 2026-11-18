-- Double authentification (phase 7, lot 7.5), facultative par compte (Q44).

CREATE TABLE user_totp (
  user_id    INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  secret     TEXT NOT NULL,             -- base32 (RFC 4648), jamais renvoyé après l'enrôlement
  enabled_at TEXT,                      -- NULL tant que le premier code n'a pas confirmé l'enrôlement
  last_step  INTEGER NOT NULL DEFAULT 0 -- dernier pas de 30 s accepté : un code ne sert qu'une fois
);

-- Codes aléatoires de 60 bits : un hash rapide suffit (choix du 2026-11-17 au lieu d'Argon2id).
CREATE TABLE user_recovery_codes (
  user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  code_hash TEXT NOT NULL,              -- BLAKE3 hex du code normalisé
  used_at   TEXT,
  PRIMARY KEY (user_id, code_hash)
);
