use super::*;
use crate::LibraryKind;

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    (dir, db)
}

async fn library(db: &Db, dir: &std::path::Path, name: &str) -> i64 {
    let root = dir.join(name);
    std::fs::create_dir(&root).unwrap();
    db.create_library(name, &root, LibraryKind::Manga)
        .await
        .unwrap()
        .id
}

async fn exec(db: &Db, sql: &'static str) {
    sqlx::query(sql).execute(db.pool()).await.unwrap();
}

#[tokio::test]
async fn a_user_is_created_with_a_temporary_password_then_found_without_case() {
    let (_dir, db) = db().await;
    let alice = db
        .create_user("Élise", "mot de passe", Role::Admin)
        .await
        .unwrap();
    assert_eq!(
        (
            alice.username.as_str(),
            alice.role,
            alice.must_change_password,
            alice.disabled
        ),
        ("Élise", Role::Admin, true, false)
    );
    assert_eq!(db.user_by_name("éLISE").await.unwrap(), Some(alice.clone()));
    assert_eq!(db.user_by_name("bob").await.unwrap(), None);
    assert!(matches!(
        db.create_user("ÉLISE", "mot de passe", Role::Reader).await,
        Err(AccountError::NameTaken(n)) if n == "ÉLISE"
    ));
    db.create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let names: Vec<_> = db
        .users()
        .await
        .unwrap()
        .into_iter()
        .map(|u| u.username)
        .collect();
    assert_eq!(names, ["bob", "Élise"]);
}

#[tokio::test]
async fn names_and_passwords_are_validated() {
    let (_dir, db) = db().await;
    for name in ["", "a b", "x/y", "<script>", &"a".repeat(33)] {
        assert!(
            matches!(
                db.create_user(name, "mot de passe", Role::Reader).await,
                Err(AccountError::InvalidName)
            ),
            "{name:?}"
        );
    }
    db.create_user(&"é".repeat(32), "mot de passe", Role::Reader)
        .await
        .unwrap();
    for password in ["court", &"x".repeat(MAX_PASSWORD + 1)] {
        assert!(matches!(
            db.create_user("bob", password, Role::Reader).await,
            Err(AccountError::WeakPassword)
        ));
    }
    assert!(
        db.users()
            .await
            .unwrap()
            .iter()
            .all(|u| u.username != "bob")
    );
}

#[tokio::test]
async fn a_reader_gets_existing_libraries_but_not_later_ones() {
    let (dir, db) = db().await;
    let manga = library(&db, dir.path(), "Manga").await;
    let bd = library(&db, dir.path(), "BD").await;
    let reader = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let admin = db
        .create_user("alice", "mot de passe", Role::Admin)
        .await
        .unwrap();
    library(&db, dir.path(), "Adulte").await;
    let mut expected = vec![manga, bd];
    expected.sort();
    assert_eq!(db.user_libraries(reader.id).await.unwrap(), expected);
    assert_eq!(
        db.user_libraries(admin.id).await.unwrap(),
        Vec::<i64>::new()
    );
}

#[tokio::test]
async fn only_the_right_password_of_an_active_account_is_accepted() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    assert_eq!(
        db.check_password("BOB", "mot de passe").await.unwrap(),
        Some(bob)
    );
    assert_eq!(
        db.check_password("bob", "Mot de passe").await.unwrap(),
        None
    );
    assert_eq!(
        db.check_password("inconnu", "mot de passe").await.unwrap(),
        None
    );
    exec(&db, "UPDATE users SET disabled_at = 'x'").await;
    assert_eq!(
        db.check_password("bob", "mot de passe").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn a_new_password_closes_every_session() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(bob.id, "Firefox").await.unwrap();
    db.set_password("Bob", "nouveau secret", false)
        .await
        .unwrap();
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);
    assert_eq!(
        db.check_password("bob", "mot de passe").await.unwrap(),
        None
    );
    let bob = db
        .check_password("bob", "nouveau secret")
        .await
        .unwrap()
        .unwrap();
    assert!(!bob.must_change_password);
    db.set_password("bob", "provisoire", true).await.unwrap();
    assert!(
        db.user_by_name("bob")
            .await
            .unwrap()
            .unwrap()
            .must_change_password
    );
    assert!(matches!(
        db.set_password("inconnu", "nouveau secret", false).await,
        Err(AccountError::UnknownUser(n)) if n == "inconnu"
    ));
    assert!(matches!(
        db.set_password("bob", "court", false).await,
        Err(AccountError::WeakPassword)
    ));
}

#[tokio::test]
async fn a_session_is_found_by_its_token_only_while_valid() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(bob.id, "Firefox").await.unwrap();
    assert!(session.token.starts_with("shiori_s_"));
    assert_eq!(session.token.len(), "shiori_s_".len() + 64);
    assert_eq!(
        db.session_user(&session.token).await.unwrap(),
        Some(Session {
            id: session.id,
            user: bob.clone(),
            renewed: false,
        })
    );
    // Le jeton en clair n'est jamais en base.
    let stored: i64 = sqlx::query("SELECT COUNT(*) AS n FROM sessions WHERE token_hash = ?")
        .bind(&session.token)
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get("n");
    assert_eq!(stored, 0);
    for wrong in ["", "shiori_s_00", "shiori_t_x", &session.token[1..]] {
        assert_eq!(db.session_user(wrong).await.unwrap(), None, "{wrong:?}");
    }

    exec(&db, "UPDATE users SET disabled_at = 'x'").await;
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);
    exec(&db, "UPDATE users SET disabled_at = NULL").await;
    exec(
        &db,
        "UPDATE sessions SET expires_at = '2000-01-01T00:00:00.000Z'",
    )
    .await;
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);

    let other = db.create_session(bob.id, "Chrome").await.unwrap();
    db.end_session(&other.token).await.unwrap();
    assert_eq!(db.session_user(&other.token).await.unwrap(), None);
    db.end_session(&other.token).await.unwrap();
}

#[tokio::test]
async fn a_session_slides_at_most_once_an_hour() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(bob.id, "Firefox").await.unwrap();
    let times = || async {
        let r = sqlx::query("SELECT last_seen_at, expires_at FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
        (
            r.get::<String, _>("last_seen_at"),
            r.get::<String, _>("expires_at"),
        )
    };
    // Vue il y a 10 minutes, expire bientôt : pas encore prolongée.
    exec(
        &db,
        "UPDATE sessions SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-10 minutes'),
           expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 day')",
    )
    .await;
    let before = times().await;
    assert!(
        !db.session_user(&session.token)
            .await
            .unwrap()
            .unwrap()
            .renewed
    );
    assert_eq!(times().await, before);
    // Vue il y a deux heures : repart pour 90 jours.
    exec(
        &db,
        "UPDATE sessions SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 hours')",
    )
    .await;
    assert!(
        db.session_user(&session.token)
            .await
            .unwrap()
            .unwrap()
            .renewed
    );
    let (seen, expires) = times().await;
    assert!(seen > before.0, "{seen} {}", before.0);
    let in_89_days: String =
        sqlx::query("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+89 days') AS t")
            .fetch_one(db.pool())
            .await
            .unwrap()
            .get("t");
    assert!(expires > in_89_days, "{expires}");
}

#[tokio::test]
async fn a_deleted_user_takes_its_sessions_and_access_along() {
    let (dir, db) = db().await;
    library(&db, dir.path(), "Manga").await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(bob.id, "Firefox").await.unwrap();
    exec(&db, "DELETE FROM users").await;
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);
    let left: i64 = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM sessions) + (SELECT COUNT(*) FROM user_libraries) AS n",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
    .get("n");
    assert_eq!(left, 0);
}

#[test]
fn temporary_passwords_are_long_unambiguous_and_different() {
    let a = temporary_password();
    assert_eq!(a.len(), 16);
    assert!(
        a.chars()
            .all(|c| c.is_ascii_alphanumeric() && !"lo01".contains(c)),
        "{a}"
    );
    assert_ne!(a, temporary_password());
}

#[tokio::test]
async fn the_first_admin_is_created_only_while_no_account_exists() {
    let (_dir, db) = db().await;
    let alice = db
        .create_first_admin("alice", "mot de passe")
        .await
        .unwrap()
        .unwrap();
    // Mot de passe choisi par la personne : rien à changer.
    assert_eq!(
        (alice.role, alice.must_change_password),
        (Role::Admin, false)
    );
    assert_eq!(
        db.create_first_admin("mallory", "mot de passe")
            .await
            .unwrap(),
        None
    );
    assert_eq!(db.users().await.unwrap(), vec![alice]);
    assert!(matches!(
        db.create_first_admin("a b", "mot de passe").await,
        Err(AccountError::InvalidName)
    ));
}

/// Une session récente se lit sans écrire : pendant un scan (longues écritures), chaque image
/// demandée ne doit pas attendre son tour pour écrire.
#[tokio::test]
async fn a_recent_session_is_read_even_while_another_write_holds_the_lock() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(bob.id, "Firefox").await.unwrap();
    let mut lock = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE app_settings SET id = id")
        .execute(&mut *lock)
        .await
        .unwrap();
    let found = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        db.session_user(&session.token),
    )
    .await
    .expect("lecture bloquée par le verrou d'écriture");
    assert_eq!(found.unwrap().unwrap().user, bob);
    lock.rollback().await.unwrap();
}
