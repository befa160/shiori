//! Liens d'invitation (version 1.4, lot 1.4a) : l'administrateur prévoit le rôle et les
//! bibliothèques ; la personne invitée choisit son nom et son mot de passe. Jeton de 256 bits,
//! seul son hash BLAKE3 est gardé ; usage unique.

use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::accounts::{hash_password, hex, insert_in, name_key, random, token_hash};
use crate::{AccountError, Db, Error, Role, User, from_db, to_db};

/// Invitation pas encore utilisée (périmée comprise) : jamais son jeton.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invite {
    pub id: i64,
    pub role: Role,
    /// Bibliothèques ouvertes au futur lecteur ; vide pour un administrateur.
    pub libraries: Vec<i64>,
    pub created_at: String,
    pub expires_at: String,
}

/// Invitation créée : le jeton en clair, montré une seule fois.
#[derive(Debug, Clone)]
pub struct NewInvite {
    pub token: String,
    pub invite: Invite,
}

/// Condition d'une invitation encore bonne : son auteur est toujours administrateur et actif
/// (un administrateur rétrogradé ou désactivé ne garde pas un lien qui le referait admin).
macro_rules! valid_author {
    () => {
        " AND i.created_by IN (SELECT id FROM users WHERE role = 'admin' AND disabled_at IS NULL)"
    };
}

/// Colonnes d'une invitation listée. Macro : sqlx n'accepte que du SQL statique (`concat!`).
macro_rules! select {
    () => {
        "SELECT i.id, i.role, i.created_at, i.expires_at,
           (SELECT group_concat(library_id) FROM invite_libraries WHERE invite_id = i.id)
             AS libraries
         FROM invites i"
    };
}

fn invite(row: &SqliteRow) -> Result<Invite, Error> {
    let libraries = row.get::<Option<&str>, _>("libraries").unwrap_or_default();
    let mut libraries: Vec<i64> = libraries
        .split(',')
        .filter_map(|n| n.parse().ok())
        .collect();
    libraries.sort_unstable();
    Ok(Invite {
        id: row.get("id"),
        role: from_db(row.get("role"))?,
        libraries,
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
    })
}

impl Db {
    /// Nouvelle invitation valable `days` jours. Un lecteur aura `libraries` (inconnues
    /// ignorées), ou toutes celles qui existent aujourd'hui si `None`, comme un compte créé à la
    /// main (Q49) ; un administrateur les voit toutes.
    pub async fn create_invite(
        &self,
        created_by: i64,
        role: Role,
        libraries: Option<Vec<i64>>,
        days: u16,
    ) -> Result<NewInvite, Error> {
        let token = hex(&random::<32>());
        let mut tx = self.pool.begin().await?;
        let sql = concat!(
            "INSERT INTO invites (token_hash, role, created_by, created_at, expires_at)
             VALUES (?, ?, ?, ",
            now!(),
            ", strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ? || ' days')) RETURNING id"
        );
        let id: i64 = sqlx::query(sql)
            .bind(token_hash(&token))
            .bind(to_db(&role))
            .bind(created_by)
            .bind(days)
            .fetch_one(&mut *tx)
            .await?
            .get("id");
        if role == Role::Reader {
            match libraries {
                None => {
                    sqlx::query(
                        "INSERT INTO invite_libraries (invite_id, library_id)
                         SELECT ?, id FROM libraries",
                    )
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                }
                Some(libraries) => {
                    for library in libraries {
                        sqlx::query(
                            "INSERT OR IGNORE INTO invite_libraries (invite_id, library_id)
                             SELECT ?, id FROM libraries WHERE id = ?",
                        )
                        .bind(id)
                        .bind(library)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }
        }
        tx.commit().await?;
        let row = sqlx::query(concat!(select!(), " WHERE i.id = ?"))
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(NewInvite {
            token,
            invite: invite(&row)?,
        })
    }

    /// Invitations pas encore utilisées (auteur toujours administrateur), périmées comprises, de la plus récente à la plus
    /// ancienne.
    pub async fn invites(&self) -> Result<Vec<Invite>, Error> {
        sqlx::query(concat!(
            select!(),
            " WHERE i.used_at IS NULL",
            valid_author!(),
            " ORDER BY i.id DESC"
        ))
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(invite)
        .collect()
    }

    /// Révoque une invitation pas encore utilisée ; `false` si elle n'existe pas (ou plus).
    pub async fn revoke_invite(&self, id: i64) -> Result<bool, Error> {
        let done = sqlx::query("DELETE FROM invites WHERE id = ? AND used_at IS NULL")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Rôle prévu par une invitation valable (ni utilisée, ni périmée, ni révoquée).
    pub async fn invite_role(&self, token: &str) -> Result<Option<Role>, Error> {
        let sql = concat!(
            "SELECT role FROM invites i
             WHERE token_hash = ? AND used_at IS NULL AND expires_at > ",
            now!(),
            valid_author!()
        );
        sqlx::query(sql)
            .bind(token_hash(token))
            .fetch_optional(&self.pool)
            .await?
            .map(|row| from_db(row.get("role")))
            .transpose()
    }

    /// Crée le compte d'une invitation valable avec le nom et le mot de passe choisis (mot de
    /// passe définitif), et la consomme ; `None` si elle n'est pas (ou plus) valable. Deux
    /// requêtes simultanées sur le même lien ne créent qu'un compte.
    pub async fn accept_invite(
        &self,
        token: &str,
        username: &str,
        password: &str,
    ) -> Result<Option<User>, AccountError> {
        let key = name_key(username)?;
        // Argon2 coûte : rien n'est calculé pour un lien qui ne vaut rien.
        if self.invite_role(token).await?.is_none() {
            return Ok(None);
        }
        let hash = hash_password(password).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let sql = concat!(
            "SELECT id, role FROM invites i
             WHERE token_hash = ? AND used_at IS NULL AND expires_at > ",
            now!(),
            valid_author!()
        );
        let Some(row) = sqlx::query(sql)
            .bind(token_hash(token))
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(None);
        };
        let invite_id: i64 = row.get("id");
        let role: Role = from_db(row.get("role"))?;
        let user_id = insert_in(&mut tx, username, &key, &hash, role, false, false)
            .await?
            .expect("insertion sans condition");
        if role == Role::Reader {
            sqlx::query("DELETE FROM user_libraries WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO user_libraries (user_id, library_id)
                 SELECT ?, library_id FROM invite_libraries WHERE invite_id = ?",
            )
            .bind(user_id)
            .bind(invite_id)
            .execute(&mut *tx)
            .await?;
        }
        let sql = concat!(
            "UPDATE invites SET used_at = ",
            now!(),
            ", used_by = ? WHERE id = ?"
        );
        sqlx::query(sql)
            .bind(user_id)
            .bind(invite_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(self.user_by_id(user_id).await?)
    }
}

#[cfg(test)]
mod tests;
