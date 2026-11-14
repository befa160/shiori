//! Réglages de l'installation (lot i18n). Un seul jeu de réglages tant qu'il n'y a pas de
//! comptes ; l'interface traduit elle-même ses textes, le serveur ne garde que la préférence.

use serde::{Deserialize, Serialize};
use shiori_meta::SourceId;
use sqlx::Row;

use crate::{Db, Error, PageTurn, from_db, to_db};

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Fr,
    En,
}

/// Thème de l'interface (Q40) ; le lecteur reste sombre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Sombre ou clair selon l'appareil (`prefers-color-scheme`).
    #[default]
    System,
    Dark,
    Light,
    /// Noir pur, pour les écrans OLED.
    Oled,
    Sepia,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Settings {
    /// `None` : personne n'a choisi, l'interface suit la langue du navigateur.
    pub language: Option<Language>,
    /// Sens par défaut pour tourner les pages ; une série peut avoir le sien.
    pub page_turn: PageTurn,
    /// Métadonnées web autorisées (désactivées par défaut, Q33).
    pub metadata_online: bool,
    /// Ordre choisi des sources ; `None` : d'après la langue (voir [`Settings::source_order`]).
    pub metadata_sources: Option<Vec<SourceId>>,
    /// Fiches pour adultes permises dans les recherches (exclues par défaut).
    pub metadata_adult: bool,
    pub theme: Theme,
}

impl Settings {
    /// Ordre des sources appliqué (Q28) : celui choisi, sinon AniList d'abord en anglais et
    /// MangaDex d'abord en français (ses résumés existent souvent en français).
    pub fn source_order(&self) -> Vec<SourceId> {
        self.metadata_sources
            .clone()
            .unwrap_or_else(|| match self.language {
                Some(Language::En) => vec![SourceId::Anilist, SourceId::Mangadex],
                _ => vec![SourceId::Mangadex, SourceId::Anilist],
            })
    }

    /// Code de la langue de l'interface pour choisir un titre ou un résumé (`fr` par défaut,
    /// la langue source de l'interface).
    pub fn lang(&self) -> &'static str {
        match self.language {
            Some(Language::En) => "en",
            _ => "fr",
        }
    }
}

impl Db {
    pub async fn settings(&self) -> Result<Settings, Error> {
        let row = sqlx::query(
            "SELECT language, page_turn, metadata_online, metadata_sources, metadata_adult, theme
             FROM app_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(Settings {
            language: row
                .get::<Option<&str>, _>("language")
                .map(from_db)
                .transpose()?,
            page_turn: from_db(row.get("page_turn"))?,
            metadata_online: row.get("metadata_online"),
            metadata_adult: row.get("metadata_adult"),
            theme: from_db(row.get("theme"))?,
            metadata_sources: row
                .get::<Option<&str>, _>("metadata_sources")
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| Error::Decode(Box::new(e)))?,
        })
    }

    pub async fn set_metadata_adult(&self, adult: bool) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET metadata_adult = ? WHERE id = 1")
            .bind(adult)
            .execute(&self.pool)
            .await?;
        self.settings().await
    }

    pub async fn set_metadata_online(&self, online: bool) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET metadata_online = ? WHERE id = 1")
            .bind(online)
            .execute(&self.pool)
            .await?;
        self.settings().await
    }

    /// Ordre des sources ; `None` : d'après la langue. Recalcule les métadonnées fusionnées.
    pub async fn set_metadata_sources(
        &self,
        sources: Option<Vec<SourceId>>,
    ) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET metadata_sources = ? WHERE id = 1")
            .bind(sources.map(|s| serde_json::to_string(&s).expect("une liste se sérialise")))
            .execute(&self.pool)
            .await?;
        self.refresh_metadata(None).await?;
        self.settings().await
    }

    pub async fn set_theme(&self, theme: Theme) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET theme = ? WHERE id = 1")
            .bind(to_db(&theme))
            .execute(&self.pool)
            .await?;
        self.settings().await
    }

    pub async fn set_page_turn(&self, page_turn: PageTurn) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET page_turn = ? WHERE id = 1")
            .bind(to_db(&page_turn))
            .execute(&self.pool)
            .await?;
        self.settings().await
    }

    /// Change la langue ; le titre et le résumé venus du web en dépendent (Q34) : recalculés.
    pub async fn set_language(&self, language: Option<Language>) -> Result<Settings, Error> {
        sqlx::query("UPDATE app_settings SET language = ? WHERE id = 1")
            .bind(language.as_ref().map(to_db))
            .execute(&self.pool)
            .await?;
        self.refresh_metadata(None).await?;
        self.settings().await
    }
}

#[cfg(test)]
mod tests;
