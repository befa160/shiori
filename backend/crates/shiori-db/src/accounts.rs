//! Comptes, mots de passe et sessions (phase 7, lot 7.2).
//!
//! - Mots de passe en Argon2id (paramètres par défaut de la crate, recommandés par l'OWASP),
//!   hachés hors de l'exécuteur async : un hachage coûte des dizaines de millisecondes.
//! - Jeton de session : 256 bits du hasard du système ; seul son hash BLAKE3 est en base.
//! - Session glissante de 90 jours (Q47), prolongée au plus une fois par heure pour ne pas
//!   écrire en base à chaque image demandée.

use std::sync::OnceLock;

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, from_db, to_db};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: Role,
    /// Mot de passe provisoire, à changer à la prochaine connexion (Q45).
    pub must_change_password: bool,
    pub disabled: bool,
    pub created_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("nom de compte invalide : 1 à 32 lettres, chiffres, « . », « _ » ou « - »")]
    InvalidName,
    #[error("mot de passe trop court ({MIN_PASSWORD} caractères au moins) ou trop long")]
    WeakPassword,
    #[error("un compte s'appelle déjà « {0} »")]
    NameTaken(String),
    #[error("aucun compte « {0} »")]
    UnknownUser(String),
    #[error(transparent)]
    Db(#[from] Error),
}

/// Session tout juste ouverte : le jeton en clair n'existe qu'ici, jamais en base.
#[derive(Debug)]
pub struct NewSession {
    pub id: i64,
    pub token: String,
}

/// Session valide, retrouvée par son jeton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub user: User,
    /// Prolongée de 90 jours par cette requête : le cookie doit l'être aussi.
    pub renewed: bool,
}

/// Durée d'une session sans usage, en secondes (Q47), pour le cookie.
pub const SESSION_SECONDS: u64 = 90 * 24 * 3600;

const MIN_PASSWORD: usize = 8;
/// Un mot de passe de plusieurs Mo ferait travailler Argon2 pour rien.
const MAX_PASSWORD: usize = 1024;
/// Préfixe des jetons de session ; les jetons d'API auront le leur (`shiori_t_`).
const SESSION_PREFIX: &str = "shiori_s_";

fn user(row: &SqliteRow) -> Result<User, Error> {
    Ok(User {
        id: row.get("id"),
        username: row.get("username"),
        role: from_db(row.get("role"))?,
        must_change_password: row.get("must_change_password"),
        disabled: row.get::<Option<&str>, _>("disabled_at").is_some(),
        created_at: row.get("created_at"),
    })
}

/// Clé d'unicité : la casse ne distingue pas deux comptes (`Élise` = `élise`).
// shortcut: pas de normalisation Unicode (NFC) : « É » précomposé et « E » + accent font deux
// comptes ; ajouter `unicode-normalization` si des noms saisis sur des claviers différents se croisent.
fn name_key(username: &str) -> Result<String, AccountError> {
    let valid = (1..=32).contains(&username.chars().count())
        && username
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
    match valid {
        true => Ok(username.to_lowercase()),
        false => Err(AccountError::InvalidName),
    }
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("hasard du système indisponible");
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// Mot de passe provisoire : 16 caractères parmi 32 (80 bits), sans `l`, `o`, `0` ni `1`
/// qui se confondent à la lecture. 32 symboles : un octet modulo 32 reste uniforme.
pub fn temporary_password() -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijkmnpqrstuvwxyz23456789";
    random::<16>()
        .iter()
        .map(|b| ALPHABET[usize::from(b % 32)] as char)
        .collect()
}

async fn hash_password(password: &str) -> Result<String, AccountError> {
    if !(MIN_PASSWORD..=MAX_PASSWORD).contains(&password.chars().count()) {
        return Err(AccountError::WeakPassword);
    }
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
    })
    .await
    .map_err(|e| Error::Io(std::io::Error::other(e)))?
    .map_err(|e| Error::Io(std::io::Error::other(e.to_string())).into())
}

/// Hash d'un mot de passe que personne n'a : vérifié pour un nom inconnu, afin que la réponse
/// prenne le même temps qu'un vrai essai (on ne devine pas les noms de compte au chronomètre).
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        Argon2::default()
            .hash_password(&random::<32>())
            .expect("hachage d'un secret aléatoire")
            .to_string()
    })
}

async fn verify(password: &str, hash: String) -> bool {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok()
    })
    .await
    .unwrap_or(false)
}

impl Db {
    /// Crée un compte. Un lecteur reçoit l'accès à toutes les bibliothèques existantes (Q49) ;
    /// celles créées plus tard lui restent fermées jusqu'à ce qu'on les lui ouvre.
    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
        role: Role,
    ) -> Result<User, AccountError> {
        let created = self.insert_user(username, password, role, false).await?;
        Ok(created.expect("insertion sans condition"))
    }

    /// Premier administrateur (page d'installation, Q46), avec le mot de passe qu'il a choisi ;
    /// `None` si un compte existe déjà. La vérification et l'insertion ne font qu'une requête :
    /// deux installations simultanées ne créent pas deux administrateurs.
    pub async fn create_first_admin(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<User>, AccountError> {
        self.insert_user(username, password, Role::Admin, true)
            .await
    }

    /// `first` : seulement si aucun compte n'existe, et mot de passe définitif (choisi par la
    /// personne) ; sinon mot de passe provisoire.
    async fn insert_user(
        &self,
        username: &str,
        password: &str,
        role: Role,
        first: bool,
    ) -> Result<Option<User>, AccountError> {
        let key = name_key(username)?;
        let hash = hash_password(password).await?;
        let mut tx = self.pool.begin().await?;
        let sql = concat!(
            "INSERT INTO users (username, username_key, password_hash, role,
                                must_change_password, created_at, password_changed_at)
             SELECT ?, ?, ?, ?, ?, ",
            now!(),
            ", ",
            now!(),
            " WHERE ? OR NOT EXISTS (SELECT 1 FROM users) RETURNING id"
        );
        let id: i64 = match sqlx::query(sql)
            .bind(username)
            .bind(&key)
            .bind(&hash)
            .bind(to_db(&role))
            .bind(!first)
            .bind(!first)
            .fetch_optional(&mut *tx)
            .await
        {
            Ok(None) => return Ok(None),
            Ok(Some(row)) => row.get("id"),
            Err(e)
                if e.as_database_error()
                    .is_some_and(|d| d.is_unique_violation()) =>
            {
                return Err(AccountError::NameTaken(username.to_owned()));
            }
            Err(e) => return Err(e.into()),
        };
        if role == Role::Reader {
            sqlx::query(
                "INSERT INTO user_libraries (user_id, library_id) SELECT ?, id FROM libraries",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(self.user_by_name(username).await?)
    }

    pub async fn user_by_name(&self, username: &str) -> Result<Option<User>, Error> {
        sqlx::query("SELECT * FROM users WHERE username_key = ?")
            .bind(username.to_lowercase())
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(user)
            .transpose()
    }

    pub async fn users(&self) -> Result<Vec<User>, Error> {
        sqlx::query("SELECT * FROM users ORDER BY username_key")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(user)
            .collect()
    }

    /// Le compte si le mot de passe est le bon et le compte actif. Même durée de calcul pour
    /// un nom inconnu, un compte désactivé ou un mauvais mot de passe.
    pub async fn check_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<User>, Error> {
        let row = sqlx::query("SELECT id, password_hash FROM users WHERE username_key = ?")
            .bind(username.to_lowercase())
            .fetch_optional(&self.pool)
            .await?;
        let hash = row.as_ref().map_or_else(
            || dummy_hash().to_owned(),
            |r| r.get::<String, _>("password_hash"),
        );
        let good = verify(password, hash).await && row.is_some();
        match good {
            true => Ok(self.user_by_name(username).await?.filter(|u| !u.disabled)),
            false => Ok(None),
        }
    }

    /// Nouveau mot de passe ; `temporary` le fait changer à la prochaine connexion. Toutes les
    /// sessions du compte sont fermées : un appareil volé ne survit pas au changement.
    pub async fn set_password(
        &self,
        username: &str,
        password: &str,
        temporary: bool,
    ) -> Result<(), AccountError> {
        let hash = hash_password(password).await?;
        let mut tx = self.pool.begin().await?;
        let sql = concat!(
            "UPDATE users SET password_hash = ?, must_change_password = ?,
               password_changed_at = ",
            now!(),
            " WHERE username_key = ? RETURNING id"
        );
        let id: i64 = sqlx::query(sql)
            .bind(&hash)
            .bind(temporary)
            .bind(username.to_lowercase())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AccountError::UnknownUser(username.to_owned()))?
            .get("id");
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Ouvre une session de 90 jours pour le compte ; `label` décrit l'appareil.
    pub async fn create_session(&self, user_id: i64, label: &str) -> Result<NewSession, Error> {
        let token = format!("{SESSION_PREFIX}{}", hex(&random::<32>()));
        let sql = concat!(
            "INSERT INTO sessions (token_hash, user_id, label, created_at, last_seen_at, expires_at)
             VALUES (?, ?, ?, ",
            now!(),
            ", ",
            now!(),
            ", strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+90 days')) RETURNING id"
        );
        let id = sqlx::query(sql)
            .bind(token_hash(&token))
            .bind(user_id)
            .bind(label)
            .fetch_one(&self.pool)
            .await?
            .get("id");
        Ok(NewSession { id, token })
    }

    /// Compte et identifiant de la session si le jeton est valide (non expiré, compte actif) ;
    /// prolonge la session de 90 jours si elle n'a pas servi depuis une heure.
    pub async fn session_user(&self, token: &str) -> Result<Option<Session>, Error> {
        if !token.starts_with(SESSION_PREFIX) {
            return Ok(None);
        }
        let hash = token_hash(token);
        let sql = concat!(
            "SELECT s.id AS session_id, u.* FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = ? AND u.disabled_at IS NULL AND s.expires_at > ",
            now!()
        );
        let Some(row) = sqlx::query(sql)
            .bind(&hash)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let sql = concat!(
            "UPDATE sessions SET last_seen_at = ",
            now!(),
            ", expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+90 days')
             WHERE token_hash = ?
               AND last_seen_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')"
        );
        let renewed = sqlx::query(sql)
            .bind(&hash)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0;
        Ok(Some(Session {
            id: row.get("session_id"),
            user: user(&row)?,
            renewed,
        }))
    }

    /// Ferme la session de ce jeton (déconnexion) ; sans effet si elle n'existe plus.
    pub async fn end_session(&self, token: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Bibliothèques ouvertes explicitement au compte (un administrateur voit tout, sans ligne).
    pub async fn user_libraries(&self, user_id: i64) -> Result<Vec<i64>, Error> {
        Ok(sqlx::query(
            "SELECT library_id FROM user_libraries WHERE user_id = ? ORDER BY library_id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| r.get("library_id"))
        .collect())
    }
}

#[cfg(test)]
mod tests;
