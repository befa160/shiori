-- Marque-pages (1.2c) : par compte, au plus un par page, avec une note. La page est désignée par
-- son ID (principe 1) : un fichier modifié donne de nouvelles pages, et ses marque-pages partent
-- avec les anciennes, comme la progression.
CREATE TABLE bookmarks (
  id         INTEGER PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  page_id    TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  note       TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  UNIQUE (user_id, page_id)
);
CREATE INDEX bookmarks_page ON bookmarks(page_id);
