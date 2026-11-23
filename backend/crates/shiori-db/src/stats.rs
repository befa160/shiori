//! Statistiques de lecture (1.1a) : l'activité par jour que remplit chaque progression
//! enregistrée (`reading_activity`), strictement personnelle (Q61).

use serde::Serialize;
use sqlx::Row;

use crate::{Db, Error, ReadingStatus, Viewer};

/// Ce qu'un compte a lu sur une période.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Activity {
    pub pages: i64,
    /// Temps de lecture estimé (pauses de 5 minutes et plus exclues).
    pub seconds: i64,
    /// Tomes dont la dernière page a été atteinte.
    pub finished: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DayActivity {
    /// `AAAA-MM-JJ`, dans le fuseau de l'appareil qui a enregistré la lecture.
    pub day: String,
    #[serde(flatten)]
    pub activity: Activity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadingStats {
    /// Jours lus de la période demandée, du plus ancien au plus récent (jours sans lecture absents).
    pub days: Vec<DayActivity>,
    /// Depuis le début.
    pub total: Activity,
    /// Séries dont tous les tomes lisibles sont lus, parmi celles que le compte voit.
    pub series_finished: usize,
}

impl Db {
    /// Statistiques du compte sur les `days` derniers jours, aujourd'hui compris, aujourd'hui
    /// étant pris à `utc_offset` minutes d'UTC.
    pub async fn reading_stats(
        &self,
        viewer: &Viewer,
        days: u32,
        utc_offset: i32,
    ) -> Result<ReadingStats, Error> {
        let rows = sqlx::query(
            "SELECT day, pages, seconds, finished FROM reading_activity
             WHERE user_id = ?1 AND day > date('now', ?2, ?3) ORDER BY day",
        )
        .bind(viewer.user)
        .bind(format!("{utc_offset} minutes"))
        .bind(format!("-{days} days"))
        .fetch_all(&self.pool)
        .await?;
        let total = sqlx::query(
            "SELECT COALESCE(sum(pages), 0) AS pages, COALESCE(sum(seconds), 0) AS seconds,
                    COALESCE(sum(finished), 0) AS finished
             FROM reading_activity WHERE user_id = ?",
        )
        .bind(viewer.user)
        .fetch_one(&self.pool)
        .await?;
        let series_finished = self
            .all_series(viewer)
            .await?
            .iter()
            .filter(|s| s.reading_status == ReadingStatus::Read)
            .count();
        Ok(ReadingStats {
            days: rows
                .iter()
                .map(|r| DayActivity {
                    day: r.get("day"),
                    activity: activity(r),
                })
                .collect(),
            total: activity(&total),
            series_finished,
        })
    }
}

fn activity(row: &sqlx::sqlite::SqliteRow) -> Activity {
    Activity {
        pages: row.get("pages"),
        seconds: row.get("seconds"),
        finished: row.get("finished"),
    }
}
