//! Collections partagées (1.3d) : un nom et des séries, gérés par l'administrateur. Un compte ne
//! voit que les séries de ses bibliothèques, et pas une collection dont il ne voit aucune série
//! (l'administrateur les voit toutes, vides comprises, pour les gérer).

use std::collections::HashMap;

use serde::Serialize;
use sqlx::Row;

use crate::reading::visible;
use crate::{Db, Error, Viewer};

/// Nom le plus long accepté, en caractères.
pub const MAX_COLLECTION_NAME: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    /// Séries visibles du compte ; titres, couvertures et tri viennent de la liste des séries.
    pub series_ids: Vec<i64>,
}

/// Une collection dans le détail d'une série.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("nom vide ou de plus de {MAX_COLLECTION_NAME} caractères")]
    InvalidName,
    #[error("une collection s'appelle déjà « {0} »")]
    NameTaken(String),
    #[error(transparent)]
    Db(#[from] Error),
}

/// Nom nettoyé (espaces des bords retirés), refusé vide ou trop long.
fn clean_name(name: &str) -> Result<&str, CollectionError> {
    let name = name.trim();
    match name.is_empty() || name.chars().count() > MAX_COLLECTION_NAME {
        true => Err(CollectionError::InvalidName),
        false => Ok(name),
    }
}

fn taken(e: Error, name: &str) -> CollectionError {
    match e
        .as_database_error()
        .is_some_and(|d| d.is_unique_violation())
    {
        true => CollectionError::NameTaken(name.to_owned()),
        false => e.into(),
    }
}

impl Db {
    /// Collections que le compte voit, par nom.
    pub async fn collections(&self, viewer: &Viewer) -> Result<Vec<Collection>, Error> {
        let mut series: HashMap<i64, Vec<i64>> = HashMap::new();
        let rows = sqlx::query(concat!(
            "SELECT cs.collection_id, cs.series_id FROM collection_series cs
             JOIN series s ON s.id = cs.series_id WHERE ",
            visible!("s.library_id", "?1", "?2"),
            " ORDER BY cs.series_id"
        ))
        .bind(viewer.admin)
        .bind(viewer.user)
        .fetch_all(&self.pool)
        .await?;
        for row in &rows {
            series
                .entry(row.get("collection_id"))
                .or_default()
                .push(row.get("series_id"));
        }
        let all = sqlx::query("SELECT id, name FROM collections ORDER BY name, id")
            .fetch_all(&self.pool)
            .await?;
        Ok(all
            .iter()
            .filter_map(|row| {
                let id: i64 = row.get("id");
                let series_ids = series.remove(&id).unwrap_or_default();
                (viewer.admin || !series_ids.is_empty()).then(|| Collection {
                    id,
                    name: row.get("name"),
                    series_ids,
                })
            })
            .collect())
    }

    /// Collections d'une série (visible, vérifié par l'appelant), par nom.
    pub(crate) async fn series_collections(
        &self,
        series: i64,
    ) -> Result<Vec<CollectionRef>, Error> {
        let rows = sqlx::query(
            "SELECT c.id, c.name FROM collections c
             JOIN collection_series cs ON cs.collection_id = c.id
             WHERE cs.series_id = ? ORDER BY c.name, c.id",
        )
        .bind(series)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| CollectionRef {
                id: r.get("id"),
                name: r.get("name"),
            })
            .collect())
    }

    pub async fn create_collection(&self, name: &str) -> Result<i64, CollectionError> {
        let name = clean_name(name)?;
        let row = sqlx::query(concat!(
            "INSERT INTO collections (name, created_at) VALUES (?, ",
            now!(),
            ") RETURNING id"
        ))
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| taken(e, name))?;
        Ok(row.get("id"))
    }

    /// Renomme ; `false` si la collection n'existe pas.
    pub async fn rename_collection(&self, id: i64, name: &str) -> Result<bool, CollectionError> {
        let name = clean_name(name)?;
        let done = sqlx::query("UPDATE collections SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| taken(e, name))?;
        Ok(done.rows_affected() == 1)
    }

    /// Supprime la collection (pas ses séries) ; `false` si elle n'existe pas.
    pub async fn delete_collection(&self, id: i64) -> Result<bool, Error> {
        let done = sqlx::query("DELETE FROM collections WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() == 1)
    }

    /// Ajoute une série ; `false` si la collection ou la série n'existe pas. Déjà dedans : rien.
    pub async fn add_to_collection(&self, id: i64, series: i64) -> Result<bool, Error> {
        let done = sqlx::query(
            "INSERT OR IGNORE INTO collection_series (collection_id, series_id)
             SELECT c.id, s.id FROM collections c, series s WHERE c.id = ? AND s.id = ?",
        )
        .bind(id)
        .bind(series)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() == 1 || self.in_collection(id, series).await?)
    }

    /// Retire une série ; `false` si la collection n'existe pas.
    pub async fn remove_from_collection(&self, id: i64, series: i64) -> Result<bool, Error> {
        sqlx::query("DELETE FROM collection_series WHERE collection_id = ? AND series_id = ?")
            .bind(id)
            .bind(series)
            .execute(&self.pool)
            .await?;
        Ok(sqlx::query("SELECT 1 FROM collections WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .is_some())
    }

    async fn in_collection(&self, id: i64, series: i64) -> Result<bool, Error> {
        Ok(
            sqlx::query(
                "SELECT 1 FROM collection_series WHERE collection_id = ? AND series_id = ?",
            )
            .bind(id)
            .bind(series)
            .fetch_optional(&self.pool)
            .await?
            .is_some(),
        )
    }
}

#[cfg(test)]
mod tests;
