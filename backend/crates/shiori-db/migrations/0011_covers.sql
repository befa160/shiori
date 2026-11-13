-- Couvertures venues d'une source web (lot 5.6, Q32), téléchargées par le serveur : gardées
-- dans la base (persistantes, sauvegardées avec elle), identifiées par le hash BLAKE3 de leur
-- contenu. Une série en choisit une par `cover_image_id` (champ édité de `series_metadata`).
CREATE TABLE covers (
  id         TEXT PRIMARY KEY,
  media_type TEXT NOT NULL,
  width      INTEGER NOT NULL,
  height     INTEGER NOT NULL,
  data       BLOB NOT NULL,
  created_at TEXT NOT NULL
);
