-- Jetons d'API (phase 7, lot 7.6, Q43) : lecture et progression seulement, pour d'autres
-- logiciels (liseuse, OPDS plus tard). Seul le hash est gardé.
CREATE TABLE api_tokens (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  token_hash   TEXT NOT NULL UNIQUE,  -- BLAKE3 hex du jeton
  prefix       TEXT NOT NULL,         -- 8 premiers caractères après `shiori_t_`, pour le reconnaître
  created_at   TEXT NOT NULL,
  last_used_at TEXT,
  expires_at   TEXT                   -- NULL : sans expiration
);
CREATE INDEX api_tokens_user ON api_tokens(user_id);
