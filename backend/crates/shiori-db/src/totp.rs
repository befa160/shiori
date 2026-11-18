//! Double authentification (phase 7, lot 7.5) : TOTP RFC 6238 (SHA-1, 6 chiffres, pas de 30 s,
//! tolérance d'un pas), et 10 codes de secours à usage unique.
//!
//! - L'enrôlement n'est actif qu'après un premier code juste : un QR mal scanné ne bloque pas
//!   le compte.
//! - Un code accepté ne resservira pas (`last_step`), même dans sa fenêtre de validité.
//! - Codes de secours : 12 caractères (60 bits) tirés au hasard, seul leur hash BLAKE3 est gardé.

use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;
use totp_rs::{Builder, Secret, Totp};

use crate::accounts::{random, readable, token_hash};
use crate::{Db, Error};

const ISSUER: &str = "Shiori";
const RECOVERY_CODES: usize = 10;

/// Secret à saisir dans l'application d'authentification (ou son QR, d'après `uri`).
#[derive(Debug, Clone)]
pub struct Enrollment {
    /// Base32, pour une saisie à la main.
    pub secret: String,
    /// `otpauth://totp/Shiori:<nom>?secret=…`, contenu du QR.
    pub uri: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TotpError {
    #[error("la double authentification est déjà active")]
    AlreadyEnabled,
    #[error("aucune double authentification en cours d'activation")]
    NotStarted,
    #[error("code incorrect")]
    WrongCode,
    #[error(transparent)]
    Db(#[from] Error),
}

fn totp(secret: &str, account: &str) -> Result<Totp, Error> {
    let secret = Secret::try_from_base32(secret)
        .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
    Builder::new()
        .with_secret(secret)
        .with_account_name(account)
        .with_issuer(Some(ISSUER))
        .build()
        .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Le code tel que saisi, sans espaces ni tirets, en minuscules.
fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Pas de temps accepté pour ce code, s'il est juste maintenant.
fn check(secret: &str, code: &str, time: u64) -> Result<Option<u64>, Error> {
    let code = normalize(code);
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    // Le nom du compte ne sert qu'au QR : la vérification n'en dépend pas.
    Ok(totp(secret, "compte")?.check(&code, time))
}

impl Db {
    /// Vrai si le compte a la double authentification (enrôlement confirmé).
    pub async fn totp_enabled(&self, user_id: i64) -> Result<bool, Error> {
        Ok(
            sqlx::query("SELECT 1 FROM user_totp WHERE user_id = ? AND enabled_at IS NOT NULL")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
                .is_some(),
        )
    }

    /// Commence l'enrôlement : nouveau secret, inactif jusqu'à [`Db::confirm_totp`]. Recommencer
    /// remplace le secret d'un enrôlement non confirmé.
    pub async fn start_totp(&self, user_id: i64, username: &str) -> Result<Enrollment, TotpError> {
        if self.totp_enabled(user_id).await? {
            return Err(TotpError::AlreadyEnabled);
        }
        let secret = Secret::new_stack(random::<20>()).to_base32();
        let uri = totp(&secret, username)?
            .to_url()
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
        sqlx::query(
            "INSERT INTO user_totp (user_id, secret) VALUES (?, ?)
             ON CONFLICT (user_id) DO UPDATE SET secret = excluded.secret, enabled_at = NULL, last_step = 0",
        )
        .bind(user_id)
        .bind(&secret)
        .execute(&self.pool)
        .await?;
        Ok(Enrollment { secret, uri })
    }

    /// Confirme l'enrôlement par un premier code ; renvoie les codes de secours, montrés une fois.
    pub async fn confirm_totp(&self, user_id: i64, code: &str) -> Result<Vec<String>, TotpError> {
        self.confirm_totp_at(user_id, code, now()).await
    }

    pub(crate) async fn confirm_totp_at(
        &self,
        user_id: i64,
        code: &str,
        time: u64,
    ) -> Result<Vec<String>, TotpError> {
        let secret: String =
            sqlx::query("SELECT secret FROM user_totp WHERE user_id = ? AND enabled_at IS NULL")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(TotpError::NotStarted)?
                .get("secret");
        let step = check(&secret, code, time)?.ok_or(TotpError::WrongCode)?;
        let codes: Vec<String> = (0..RECOVERY_CODES)
            .map(|_| {
                let raw = readable::<12>();
                format!("{}-{}-{}", &raw[..4], &raw[4..8], &raw[8..])
            })
            .collect();
        let mut tx = self.pool.begin().await?;
        sqlx::query(concat!(
            "UPDATE user_totp SET enabled_at = ",
            now!(),
            ", last_step = ? WHERE user_id = ?"
        ))
        .bind(i64::try_from(step).unwrap_or(i64::MAX))
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        for code in &codes {
            sqlx::query("INSERT INTO user_recovery_codes (user_id, code_hash) VALUES (?, ?)")
                .bind(user_id)
                .bind(token_hash(&normalize(code)))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(codes)
    }

    /// Second facteur : un code TOTP (6 chiffres, jamais deux fois le même) ou un code de
    /// secours non utilisé, qui est alors consommé. Faux si le compte n'a pas la double
    /// authentification.
    pub async fn check_second_factor(&self, user_id: i64, code: &str) -> Result<bool, Error> {
        self.check_second_factor_at(user_id, code, now()).await
    }

    pub(crate) async fn check_second_factor_at(
        &self,
        user_id: i64,
        code: &str,
        time: u64,
    ) -> Result<bool, Error> {
        let Some(row) = sqlx::query(
            "SELECT secret FROM user_totp WHERE user_id = ? AND enabled_at IS NOT NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(false);
        };
        if let Some(step) = check(row.get("secret"), code, time)? {
            // Conditionnel : deux requêtes simultanées avec le même code, une seule passe.
            let step = i64::try_from(step).unwrap_or(i64::MAX);
            let done = sqlx::query(
                "UPDATE user_totp SET last_step = ? WHERE user_id = ? AND last_step < ?",
            )
            .bind(step)
            .bind(user_id)
            .bind(step)
            .execute(&self.pool)
            .await?;
            return Ok(done.rows_affected() > 0);
        }
        let done = sqlx::query(concat!(
            "UPDATE user_recovery_codes SET used_at = ",
            now!(),
            " WHERE user_id = ? AND code_hash = ? AND used_at IS NULL"
        ))
        .bind(user_id)
        .bind(token_hash(&normalize(code)))
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Codes de secours encore utilisables.
    pub async fn recovery_codes_left(&self, user_id: i64) -> Result<i64, Error> {
        Ok(sqlx::query(
            "SELECT COUNT(*) AS n FROM user_recovery_codes WHERE user_id = ? AND used_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?
        .get("n"))
    }

    /// Retire la double authentification (ou un enrôlement en cours) ; faux s'il n'y en avait pas.
    pub async fn disable_totp(&self, user_id: i64) -> Result<bool, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM user_totp WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(done.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests;
