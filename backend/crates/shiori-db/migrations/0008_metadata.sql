-- Métadonnées locales d'un tome (lot 5.2) : `ComicInfo.xml` et `index.json` à la racine de
-- l'archive, normalisés (`shiori_core::LocalMetadata` en JSON). Réécrites à chaque scan du tome.
CREATE TABLE volume_metadata (
  volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  source    TEXT NOT NULL CHECK (source IN ('comicinfo','index_json')),
  data      TEXT NOT NULL,
  PRIMARY KEY (volume_id, source)
);
