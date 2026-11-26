//! « À lire » (1.3c) : séries mises de côté par un compte, montrées à l'accueil dans l'ordre
//! d'ajout (`to_read_at` des séries). Une série en sort dès qu'on la commence ([`Db::save_progress_at`])
//! ou qu'on la marque lue.

use crate::{Db, Error, Viewer};

impl Db {
    /// Met une série visible dans « À lire » ; `false` si elle ne l'est pas. Déjà dedans : la date
    /// d'ajout ne change pas.
    pub async fn add_to_read(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        if !self.series_visible(viewer, series).await? {
            return Ok(false);
        }
        sqlx::query(concat!(
            "INSERT INTO to_read (user_id, series_id, added_at) VALUES (?, ?, ",
            now!(),
            ") ON CONFLICT DO NOTHING"
        ))
        .bind(viewer.user)
        .bind(series)
        .execute(&self.pool)
        .await?;
        Ok(true)
    }

    /// Retire une série visible de « À lire » ; `false` si elle ne l'est pas.
    pub async fn remove_to_read(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        if !self.series_visible(viewer, series).await? {
            return Ok(false);
        }
        sqlx::query("DELETE FROM to_read WHERE user_id = ? AND series_id = ?")
            .bind(viewer.user)
            .bind(series)
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    pub(crate) async fn in_to_read(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        Ok(
            sqlx::query("SELECT 1 FROM to_read WHERE user_id = ? AND series_id = ?")
                .bind(viewer.user)
                .bind(series)
                .fetch_optional(&self.pool)
                .await?
                .is_some(),
        )
    }
}

#[cfg(test)]
mod tests;
