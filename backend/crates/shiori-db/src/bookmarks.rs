//! Marque-pages (1.2c) : par compte, au plus un par page, chacun avec une note. Ils désignent la
//! page par son ID ; seul leur propriétaire les voit.

use serde::Serialize;
use shiori_core::{PageId, VolumeId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, Viewer};

/// Note la plus longue acceptée, en caractères.
pub const MAX_NOTE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bookmark {
    pub id: i64,
    pub volume_id: String,
    pub page_id: String,
    /// Position de la page dans son tome, pour « page 42 » et le tri.
    pub ordinal: i64,
    pub note: String,
    pub created_at: String,
}

impl Bookmark {
    fn of(row: &SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            volume_id: row.get("volume_id"),
            page_id: row.get("page_id"),
            ordinal: row.get("ordinal"),
            note: row.get("note"),
            created_at: row.get("created_at"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddBookmark {
    Added(Bookmark),
    /// La page avait déjà un marque-page de ce compte : il est rendu tel quel.
    Existing(Bookmark),
    UnknownVolume,
    PageNotInVolume,
}

/// Note nettoyée (espaces des bords retirés) ; `None` si elle dépasse [`MAX_NOTE`].
pub fn clean_note(note: &str) -> Option<String> {
    let note = note.trim();
    (note.chars().count() <= MAX_NOTE).then(|| note.to_owned())
}

macro_rules! bookmarks_where {
    ($filter:literal) => {
        concat!(
            "SELECT b.id, p.volume_id, b.page_id, p.ordinal, b.note, b.created_at
             FROM bookmarks b JOIN pages p ON p.id = b.page_id
             WHERE b.user_id = ?1 AND ",
            $filter
        )
    };
}

impl Db {
    /// Marque-pages du compte dans un tome, dans l'ordre des pages ; `None` si le tome lui est
    /// invisible.
    pub async fn bookmarks_of(
        &self,
        viewer: &Viewer,
        volume: &VolumeId,
    ) -> Result<Option<Vec<Bookmark>>, Error> {
        if !self.volume_visible(viewer, volume).await? {
            return Ok(None);
        }
        let rows = sqlx::query(bookmarks_where!("p.volume_id = ?2 ORDER BY p.ordinal"))
            .bind(viewer.user)
            .bind(volume.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(Some(rows.iter().map(Bookmark::of).collect()))
    }

    /// Marque-pages du compte dans les tomes d'une série (visible, vérifié par l'appelant),
    /// rangés par tome puis par page ; l'ordre des tomes est celui de la série, côté appelant.
    pub async fn series_bookmarks(
        &self,
        viewer: &Viewer,
        series: i64,
    ) -> Result<Vec<Bookmark>, Error> {
        let rows = sqlx::query(bookmarks_where!(
            "p.volume_id IN (SELECT volume_id FROM file_locations WHERE series_id = ?2)
             ORDER BY p.volume_id, p.ordinal"
        ))
        .bind(viewer.user)
        .bind(series)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(Bookmark::of).collect())
    }

    /// Marque une page du tome ; `note` déjà nettoyée ([`clean_note`]).
    pub async fn add_bookmark(
        &self,
        viewer: &Viewer,
        volume: &VolumeId,
        page: &PageId,
        note: &str,
    ) -> Result<AddBookmark, Error> {
        if !self.volume_visible(viewer, volume).await? {
            return Ok(AddBookmark::UnknownVolume);
        }
        let page = page.to_string();
        let in_volume = sqlx::query("SELECT 1 FROM pages WHERE id = ? AND volume_id = ?")
            .bind(&page)
            .bind(volume.to_string())
            .fetch_optional(&self.pool)
            .await?;
        if in_volume.is_none() {
            return Ok(AddBookmark::PageNotInVolume);
        }
        let added = sqlx::query(concat!(
            "INSERT INTO bookmarks (user_id, page_id, note, created_at) VALUES (?, ?, ?, ",
            now!(),
            ") ON CONFLICT (user_id, page_id) DO NOTHING"
        ))
        .bind(viewer.user)
        .bind(&page)
        .bind(note)
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1;
        let row = sqlx::query(bookmarks_where!("b.page_id = ?2"))
            .bind(viewer.user)
            .bind(&page)
            .fetch_one(&self.pool)
            .await?;
        let bookmark = Bookmark::of(&row);
        Ok(match added {
            true => AddBookmark::Added(bookmark),
            false => AddBookmark::Existing(bookmark),
        })
    }

    /// Change la note d'un marque-page du compte ; `None` s'il n'en a pas de ce numéro.
    pub async fn set_bookmark_note(
        &self,
        viewer: &Viewer,
        id: i64,
        note: &str,
    ) -> Result<Option<Bookmark>, Error> {
        sqlx::query("UPDATE bookmarks SET note = ? WHERE id = ? AND user_id = ?")
            .bind(note)
            .bind(id)
            .bind(viewer.user)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query(bookmarks_where!("b.id = ?2"))
            .bind(viewer.user)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(Bookmark::of))
    }

    /// Retire un marque-page du compte ; `false` s'il n'en a pas de ce numéro.
    pub async fn delete_bookmark(&self, viewer: &Viewer, id: i64) -> Result<bool, Error> {
        let deleted = sqlx::query("DELETE FROM bookmarks WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(viewer.user)
            .execute(&self.pool)
            .await?;
        Ok(deleted.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests;
