use super::*;
use crate::{Role, UserChange};

async fn db() -> (tempfile::TempDir, Db, i64) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let user = db
        .create_user("élise", "mot de passe", Role::Reader)
        .await
        .unwrap()
        .id;
    (dir, db, user)
}

#[tokio::test]
async fn a_token_is_shown_once_listed_without_its_secret_and_finds_its_account() {
    let (_dir, db, user) = db().await;
    let new = db.create_token(user, "Liseuse", None).await.unwrap();
    assert!(new.token.starts_with("shiori_t_") && new.token.len() == 9 + 64);
    assert_eq!(new.info.prefix, new.token[..17]);
    assert_eq!(
        (
            new.info.expires_at.as_deref(),
            new.info.last_used_at.as_deref()
        ),
        (None, None)
    );
    let listed = db.tokens(user).await.unwrap();
    assert_eq!(listed, vec![new.info.clone()]);

    let found = db.token_user(&new.token).await.unwrap().unwrap();
    assert_eq!(found.id, user);
    // Premier usage noté ; le suivant, dans l'heure, n'écrit pas.
    let used = db.tokens(user).await.unwrap()[0]
        .last_used_at
        .clone()
        .unwrap();
    db.token_user(&new.token).await.unwrap().unwrap();
    assert_eq!(db.tokens(user).await.unwrap()[0].last_used_at, Some(used));

    assert_eq!(db.token_user("shiori_t_inconnu").await.unwrap(), None);
    // Un jeton de session n'est jamais lu comme un jeton d'API, ni l'inverse.
    let session = db.create_session(user, "x").await.unwrap();
    assert_eq!(db.token_user(&session.token).await.unwrap(), None);
    assert_eq!(db.session_user(&new.token).await.unwrap(), None);
}

#[tokio::test]
async fn expired_revoked_or_disabled_tokens_give_nothing() {
    let (_dir, db, user) = db().await;
    let lasting = db.create_token(user, "un an", Some(365)).await.unwrap();
    assert!(lasting.info.expires_at.as_deref().unwrap() > lasting.info.created_at.as_str());
    let old = db.create_token(user, "vieux", Some(1)).await.unwrap();
    sqlx::query("UPDATE api_tokens SET expires_at = '2000-01-01T00:00:00.000Z' WHERE id = ?")
        .bind(old.info.id)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(db.token_user(&old.token).await.unwrap(), None);
    assert!(db.token_user(&lasting.token).await.unwrap().is_some());

    // Plus récent d'abord, périmés compris.
    let names: Vec<_> = db
        .tokens(user)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["vieux", "un an"]);

    let other = db
        .create_user("autre", "mot de passe", Role::Reader)
        .await
        .unwrap()
        .id;
    assert!(
        !db.revoke_token(other, old.info.id).await.unwrap(),
        "pas le sien"
    );
    assert!(db.revoke_token(user, old.info.id).await.unwrap());
    assert!(!db.revoke_token(user, old.info.id).await.unwrap());

    let change = UserChange {
        disabled: Some(true),
        ..UserChange::default()
    };
    db.update_user(user, change).await.unwrap();
    assert_eq!(
        db.token_user(&lasting.token).await.unwrap(),
        None,
        "compte désactivé"
    );
}
