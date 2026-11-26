//! Séries suivies (1.3a) : par compte ; leurs tomes ajoutés font les nouveautés
//! ([`Db::new_volumes`]).

use crate::{Db, Error, Viewer};

impl Db {
    /// Suit une série visible ; `false` si elle ne l'est pas. Les tomes déjà là ne sont pas
    /// nouveaux : seuls comptent ceux ajoutés après ce moment.
    pub async fn follow(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        if !self.series_visible(viewer, series).await? {
            return Ok(false);
        }
        sqlx::query(concat!(
            "INSERT INTO series_follows (user_id, series_id, seen_at) VALUES (?, ?, ",
            now!(),
            ") ON CONFLICT DO NOTHING"
        ))
        .bind(viewer.user)
        .bind(series)
        .execute(&self.pool)
        .await?;
        Ok(true)
    }

    /// Ne suit plus une série visible ; `false` si elle ne l'est pas.
    pub async fn unfollow(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        if !self.series_visible(viewer, series).await? {
            return Ok(false);
        }
        sqlx::query("DELETE FROM series_follows WHERE user_id = ? AND series_id = ?")
            .bind(viewer.user)
            .bind(series)
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    pub(crate) async fn followed(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        Ok(
            sqlx::query("SELECT 1 FROM series_follows WHERE user_id = ? AND series_id = ?")
                .bind(viewer.user)
                .bind(series)
                .fetch_optional(&self.pool)
                .await?
                .is_some(),
        )
    }

    /// « Tout marquer comme vu » : plus aucun tome déjà là n'est nouveau pour ce compte.
    pub async fn mark_new_seen(&self, viewer: &Viewer) -> Result<(), Error> {
        sqlx::query(concat!(
            "UPDATE series_follows SET seen_at = ",
            now!(),
            " WHERE user_id = ?"
        ))
        .bind(viewer.user)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
