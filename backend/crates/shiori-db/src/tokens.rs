//! Jetons d'API (phase 7, lot 7.6) : `shiori_t_` + 256 bits du hasard, seul le hash BLAKE3 est
//! gardé. Le jeton en clair n'est montré qu'à sa création.

use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::accounts::{hex, random, token_hash, user};
use crate::{Db, Error, User};

const TOKEN_PREFIX: &str = "shiori_t_";

/// Jeton tel que listé : jamais son secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiToken {
    pub id: i64,
    pub name: String,
    /// `shiori_t_` et les 8 premiers caractères : de quoi le reconnaître dans un autre logiciel.
    pub prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
}

/// Jeton créé : le secret en clair, montré une seule fois.
#[derive(Debug, Clone)]
pub struct NewToken {
    pub token: String,
    pub info: ApiToken,
}

fn api_token(row: &SqliteRow) -> ApiToken {
    ApiToken {
        id: row.get("id"),
        name: row.get("name"),
        prefix: format!("{TOKEN_PREFIX}{}", row.get::<&str, _>("prefix")),
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
        expires_at: row.get("expires_at"),
    }
}

impl Db {
    /// Nouveau jeton ; `days` : durée de validité, `None` pour sans expiration.
    pub async fn create_token(
        &self,
        user_id: i64,
        name: &str,
        days: Option<u16>,
    ) -> Result<NewToken, Error> {
        let secret = hex(&random::<32>());
        let token = format!("{TOKEN_PREFIX}{secret}");
        let sql = concat!(
            "INSERT INTO api_tokens (user_id, name, token_hash, prefix, created_at, expires_at)
             VALUES (?, ?, ?, ?, ",
            now!(),
            ", CASE WHEN ? IS NULL THEN NULL
                    ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ? || ' days') END)
             RETURNING *"
        );
        let row = sqlx::query(sql)
            .bind(user_id)
            .bind(name)
            .bind(token_hash(&token))
            .bind(&secret[..8])
            .bind(days)
            .bind(days)
            .fetch_one(&self.pool)
            .await?;
        Ok(NewToken {
            token,
            info: api_token(&row),
        })
    }

    /// Jetons du compte, du plus récent au plus ancien, périmés compris (à supprimer à la main).
    pub async fn tokens(&self, user_id: i64) -> Result<Vec<ApiToken>, Error> {
        Ok(
            sqlx::query("SELECT * FROM api_tokens WHERE user_id = ? ORDER BY id DESC")
                .bind(user_id)
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(api_token)
                .collect(),
        )
    }

    /// Supprime un jeton du compte ; `false` s'il n'est pas à lui.
    pub async fn revoke_token(&self, user_id: i64, id: i64) -> Result<bool, Error> {
        let done = sqlx::query("DELETE FROM api_tokens WHERE user_id = ? AND id = ?")
            .bind(user_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Compte d'un jeton valide (non périmé, compte actif). Dernier usage noté au plus une
    /// fois par heure, comme les sessions : pas d'écriture à chaque image.
    pub async fn token_user(&self, token: &str) -> Result<Option<User>, Error> {
        if !token.starts_with(TOKEN_PREFIX) {
            return Ok(None);
        }
        let hash = token_hash(token);
        let sql = concat!(
            "SELECT u.*, t.last_used_at IS NULL
                    OR t.last_used_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour') AS stale
             FROM api_tokens t JOIN users u ON u.id = t.user_id
             WHERE t.token_hash = ? AND u.disabled_at IS NULL
               AND (t.expires_at IS NULL OR t.expires_at > ",
            now!(),
            ")"
        );
        let Some(row) = sqlx::query(sql)
            .bind(&hash)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        if row.get::<bool, _>("stale") {
            let sql = concat!(
                "UPDATE api_tokens SET last_used_at = ",
                now!(),
                " WHERE token_hash = ?"
            );
            sqlx::query(sql).bind(&hash).execute(&self.pool).await?;
        }
        Ok(Some(user(&row)?))
    }
}

#[cfg(test)]
mod tests;
