//! Réglages de l'installation (lot i18n). Un seul jeu de réglages tant qu'il n'y a pas de
//! comptes ; l'interface traduit elle-même ses textes, le serveur ne garde que la préférence.

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{Db, Error, from_db, to_db};

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Fr,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Settings {
    /// `None` : personne n'a choisi, l'interface suit la langue du navigateur.
    pub language: Option<Language>,
}

impl Db {
    pub async fn settings(&self) -> Result<Settings, Error> {
        let row = sqlx::query("SELECT language FROM app_settings WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(Settings {
            language: row
                .get::<Option<&str>, _>("language")
                .map(from_db)
                .transpose()?,
        })
    }

    pub async fn set_language(&self, language: Option<Language>) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET language = ? WHERE id = 1")
            .bind(language.as_ref().map(to_db))
            .execute(&self.pool)
            .await?;
        self.settings().await
    }
}

#[cfg(test)]
mod tests;
