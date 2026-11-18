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
    db.set_password("Bob", "nouveau secret", false, None)
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
    db.set_password("bob", "provisoire", true, None)
        .await
        .unwrap();
    assert!(
        db.user_by_name("bob")
            .await
            .unwrap()
            .unwrap()
            .must_change_password
    );
    assert!(matches!(
        db.set_password("inconnu", "nouveau secret", false, None).await,
        Err(AccountError::UnknownUser(n)) if n == "inconnu"
    ));
    assert!(matches!(
        db.set_password("bob", "court", false, None).await,
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

/// Base d'avant les comptes : progression et réglages de série (migration 14) à remplir.
async fn before_accounts(path: &std::path::Path, admin_already: bool) {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    sqlx::migrate!().run_to(14, &pool).await.unwrap();
    sqlx::query(
        "INSERT INTO volumes VALUES ('v', 1, 'cbz', 'ok', 3, 1, '2026-11-11T00:00:00.000Z');
         INSERT INTO libraries (id, name, root, created_at) VALUES (1, 'L', '/l', '');
         INSERT INTO series (id, library_id, folder, name, created_at, reading_mode, page_turn)
           VALUES (1, 1, 'A', 'A', '', 'ltr', 'rtl');
         INSERT INTO series (id, library_id, folder, name, created_at) VALUES (2, 1, 'B', 'B', '');
         INSERT INTO reading_progress VALUES ('v', 'p', 2, 'x', '2026-11-12T00:00:00.000Z');",
    )
    .execute(&pool)
    .await
    .unwrap();
    if admin_already {
        sqlx::query(
            "INSERT INTO users (username, username_key, password_hash, role, created_at,
                                password_changed_at)
             VALUES ('cli', 'cli', 'x', 'admin', '', '')",
        )
        .execute(&pool)
        .await
        .unwrap();
    }
    pool.close().await;
}

async fn counts(db: &Db) -> (i64, i64, i64, i64) {
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM legacy_progress) AS lp,
                (SELECT COUNT(*) FROM legacy_series_settings) AS ls,
                (SELECT COUNT(*) FROM reading_progress) AS rp,
                (SELECT COUNT(*) FROM user_series_settings) AS us",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    (row.get("lp"), row.get("ls"), row.get("rp"), row.get("us"))
}

#[tokio::test]
async fn the_first_admin_takes_back_what_was_read_before_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    before_accounts(&path, false).await;
    let db = Db::open(&path).await.unwrap();
    assert_eq!(
        counts(&db).await,
        (1, 1, 0, 0),
        "gardé en attendant un administrateur"
    );

    // Un lecteur ne reprend rien ; le premier administrateur reprend tout.
    db.create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    assert_eq!(counts(&db).await, (1, 1, 0, 0));
    let alice = db
        .create_user("alice", "mot de passe", Role::Admin)
        .await
        .unwrap();
    assert_eq!(counts(&db).await, (0, 0, 1, 1));
    let viewer = db.viewer(alice.id).await.unwrap();
    let settings = db.series_settings(&viewer, 1).await.unwrap().unwrap();
    assert_eq!(
        (settings.reading_mode, settings.page_turn),
        (crate::ReadingMode::Ltr, crate::PageTurn::Rtl)
    );
    db.create_user("carol", "mot de passe", Role::Admin)
        .await
        .unwrap();
    assert_eq!(
        counts(&db).await,
        (0, 0, 1, 1),
        "le second administrateur ne reprend rien"
    );
}

#[tokio::test]
async fn an_admin_created_from_the_cli_takes_it_back_at_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    before_accounts(&path, true).await;
    let db = Db::open(&path).await.unwrap();
    assert_eq!(counts(&db).await, (0, 0, 1, 1));
}

#[tokio::test]
async fn the_last_active_admin_cannot_be_demoted_disabled_or_deleted() {
    let (dir, db) = db().await;
    let manga = library(&db, dir.path(), "Manga").await;
    let alice = db
        .create_user("alice", "mot de passe", Role::Admin)
        .await
        .unwrap();
    let demote = |role, disabled| UserChange {
        role,
        disabled,
        ..Default::default()
    };
    for change in [demote(Some(Role::Reader), None), demote(None, Some(true))] {
        assert!(matches!(
            db.update_user(alice.id, change).await,
            Err(AccountError::LastAdmin)
        ));
    }
    assert!(matches!(
        db.delete_user(alice.id).await,
        Err(AccountError::LastAdmin)
    ));
    assert_eq!(
        db.update_user(9999, UserChange::default()).await.unwrap(),
        None
    );
    assert!(!db.delete_user(9999).await.unwrap());

    // Avec un second administrateur, l'un peut devenir lecteur et recevoir des bibliothèques.
    let bob = db
        .create_user("bob", "mot de passe", Role::Admin)
        .await
        .unwrap();
    let session = db.create_session(alice.id, "Firefox").await.unwrap();
    let change = UserChange {
        role: Some(Role::Reader),
        libraries: Some(vec![manga, 4242]),
        ..Default::default()
    };
    let alice = db.update_user(alice.id, change).await.unwrap().unwrap();
    assert_eq!(alice.role, Role::Reader);
    assert_eq!(
        db.user_libraries(alice.id).await.unwrap(),
        [manga],
        "inconnue ignorée"
    );
    assert!(db.session_user(&session.token).await.unwrap().is_some());

    // Désactivé : ses sessions sont fermées ; réactivé, il peut se reconnecter.
    let off = db
        .update_user(alice.id, demote(None, Some(true)))
        .await
        .unwrap()
        .unwrap();
    assert!(off.disabled);
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);
    let on = db
        .update_user(alice.id, demote(None, Some(false)))
        .await
        .unwrap()
        .unwrap();
    assert!(!on.disabled);
    assert!(
        db.check_password("alice", "mot de passe")
            .await
            .unwrap()
            .is_some()
    );

    assert!(db.delete_user(alice.id).await.unwrap());
    assert_eq!(db.user_by_id(alice.id).await.unwrap(), None);
    assert!(matches!(
        db.delete_user(bob.id).await,
        Err(AccountError::LastAdmin)
    ));
}

#[tokio::test]
async fn devices_are_listed_and_closed_one_by_one_or_all_but_the_current() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let phone = db.create_session(bob.id, "Téléphone").await.unwrap();
    let tablet = db.create_session(bob.id, "Tablette").await.unwrap();
    let laptop = db.create_session(bob.id, "Portable").await.unwrap();
    let labels: Vec<String> = db
        .devices(bob.id)
        .await
        .unwrap()
        .into_iter()
        .map(|d| d.label)
        .collect();
    assert_eq!(labels, ["Portable", "Tablette", "Téléphone"]);
    assert!(db.revoke_device(bob.id, tablet.id).await.unwrap());
    assert!(!db.revoke_device(bob.id, tablet.id).await.unwrap());
    assert!(
        !db.revoke_device(bob.id + 1, phone.id).await.unwrap(),
        "pas à lui"
    );

    // Changer son mot de passe ferme les autres appareils, pas celui d'où on le change.
    db.set_password("bob", "nouveau secret", false, Some(phone.id))
        .await
        .unwrap();
    assert!(db.session_user(&phone.token).await.unwrap().is_some());
    assert_eq!(db.session_user(&laptop.token).await.unwrap(), None);
    // … mais garde ses jetons d'API (la liseuse ne casse pas à chaque changement) ; couper tous
    // les appareils (perte, vol) les supprime aussi.
    let reader = db.create_token(bob.id, "liseuse", None).await.unwrap();
    db.set_password("bob", "autre secret", false, Some(phone.id))
        .await
        .unwrap();
    assert!(db.token_user(&reader.token).await.unwrap().is_some());
    db.revoke_devices(bob.id).await.unwrap();
    assert!(db.devices(bob.id).await.unwrap().is_empty());
    assert_eq!(db.token_user(&reader.token).await.unwrap(), None);
    assert!(db.tokens(bob.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn preferences_override_the_instance_defaults() {
    let (_dir, db) = db().await;
    let bob = db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    assert_eq!(
        db.preferences(bob.id).await.unwrap(),
        Preferences::default()
    );
    let chosen = Preferences {
        language: Some(Language::En),
        theme: Some(Theme::Oled),
        page_turn: Some(PageTurn::Rtl),
    };
    assert_eq!(db.set_preferences(bob.id, chosen).await.unwrap(), chosen);
    assert_eq!(db.viewer(bob.id).await.unwrap().language, Language::En);
    let back = Preferences {
        language: None,
        ..chosen
    };
    assert_eq!(db.set_preferences(bob.id, back).await.unwrap(), back);
    assert_eq!(db.viewer(bob.id).await.unwrap().language, Language::Fr);
}
