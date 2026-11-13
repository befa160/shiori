-- « Tourner les pages » (lot 4.11) : le sens des taps, du balayage, des flèches et du curseur,
-- séparé du sens de lecture (qui garde l'ordre des doubles pages et des moitiés coupées).
-- `reading` : comme le sens de lecture ; `ltr` : toucher à droite avance ; `rtl` : à gauche.
ALTER TABLE app_settings ADD COLUMN page_turn TEXT NOT NULL DEFAULT 'reading'
  CHECK (page_turn IN ('reading','ltr','rtl'));
-- Réglage de la série ; NULL = défaut global.
ALTER TABLE series ADD COLUMN page_turn TEXT CHECK (page_turn IN ('reading','ltr','rtl'));
