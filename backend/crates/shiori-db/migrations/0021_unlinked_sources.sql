-- Source déliée par l'utilisateur (lot 10.2) : la ligne reste, sans fiche, pour que
-- l'identification automatique ne la relie pas ; un lien choisi à la main l'efface.
ALTER TABLE series_sources ADD COLUMN unlinked INTEGER NOT NULL DEFAULT 0;
