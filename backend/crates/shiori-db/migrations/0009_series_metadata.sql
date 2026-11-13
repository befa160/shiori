-- Métadonnées éditables (lot 5.3).
-- `edited` : champs édités à la main (objet JSON champ → valeur, `null` compris). Un champ
--   présent est verrouillé : aucune source ne le remplace ; le retirer le rend aux sources.
-- `merged` : vue fusionnée d'une série (édité > fichiers locaux), recalculée après chaque
--   passe de scan, chaque modification et à l'ouverture de la base. C'est elle que lisent
--   les listes et le détail ; `series.publication_status` en garde le statut (filtres).
CREATE TABLE series_metadata (
  series_id INTEGER PRIMARY KEY REFERENCES series(id) ON DELETE CASCADE,
  edited    TEXT NOT NULL DEFAULT '{}',
  merged    TEXT NOT NULL DEFAULT '{}'
);
-- Le statut réglé à la main en phase 4 devient un champ édité, donc verrouillé.
INSERT INTO series_metadata (series_id, edited)
  SELECT id, json_object('publication_status', publication_status) FROM series
  WHERE publication_status IS NOT NULL;

-- Champs édités d'un tome, même règle ; sa vue fusionnée se calcule à la lecture.
CREATE TABLE volume_edits (
  volume_id TEXT PRIMARY KEY REFERENCES volumes(id) ON DELETE CASCADE,
  edited    TEXT NOT NULL DEFAULT '{}'
);
