-- Lot 8.3 : le dictionnaire `Info` d'un PDF devient une source locale (`pdf`). SQLite ne
-- modifie pas une contrainte CHECK : la table est recréée, ses lignes recopiées.
CREATE TABLE volume_metadata_new (
  volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
  source    TEXT NOT NULL CHECK (source IN ('comicinfo','index_json','pdf')),
  data      TEXT NOT NULL,
  PRIMARY KEY (volume_id, source)
);
INSERT INTO volume_metadata_new SELECT volume_id, source, data FROM volume_metadata;
DROP TABLE volume_metadata;
ALTER TABLE volume_metadata_new RENAME TO volume_metadata;
