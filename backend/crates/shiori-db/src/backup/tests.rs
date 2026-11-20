use super::*;
use crate::Role;

fn names(db: &[crate::User]) -> Vec<&str> {
    db.iter().map(|u| u.username.as_str()).collect()
}

/// Base ramenée avant la dernière migration, comme laissée par la version précédente.
async fn previous_version(path: &Path) {
    let db = Db::open(path).await.unwrap();
    db.create_user("ancien", "mot de passe", Role::Admin)
        .await
        .unwrap();
    // Défait la dernière migration (21) : retire sa colonne. À recaler sur chaque nouvelle
    // migration.
    for undo in [
        "ALTER TABLE series_sources DROP COLUMN unlinked",
        "DELETE FROM _sqlx_migrations WHERE version = 21",
    ] {
        sqlx::query(undo).execute(db.pool()).await.unwrap();
    }
    db.pool().close().await;
}

#[tokio::test]
async fn a_backup_restores_accounts_and_closes_every_session() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("shiori.db")).await.unwrap();
    let alice = db
        .create_user("alice", "mot de passe", Role::Admin)
        .await
        .unwrap();
    let saved = db
        .backup(&dir.path().join("backups"), "shiori")
        .await
        .unwrap();
    let name = saved.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("shiori-2") && name.ends_with(".db"),
        "{name}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&saved).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let original = std::fs::read(&saved).unwrap();

    db.create_user("bruno", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = db.create_session(alice.id, "x").await.unwrap();
    db.restore(&saved, dir.path()).await.unwrap();
    assert_eq!(names(&db.users().await.unwrap()), ["alice"]);
    assert_eq!(db.session_user(&session.token).await.unwrap(), None);
    // La sauvegarde n'est pas touchée, la copie de travail est effacée.
    assert_eq!(std::fs::read(&saved).unwrap(), original);
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_string_lossy().starts_with(".tmp-"))
        .collect();
    assert_eq!(left, Vec::<std::ffi::OsString>::new());
}

#[tokio::test]
async fn anything_but_a_shiori_backup_is_refused_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("shiori.db")).await.unwrap();
    db.create_user("alice", "mot de passe", Role::Admin)
        .await
        .unwrap();

    let junk = dir.path().join("junk.db");
    std::fs::write(&junk, b"pas une base").unwrap();
    let other = dir.path().join("autre.db");
    let mut conn = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&other)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE notes (x)")
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    // Même migrations, schéma modifié à la main.
    let tweaked = db.backup(dir.path(), "retouchée").await.unwrap();
    let mut conn = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&tweaked))
        .await
        .unwrap();
    sqlx::query("CREATE INDEX en_trop ON users(username)")
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();

    for file in [&junk, &other, &tweaked, &dir.path().join("absent.db")] {
        let refused = db.restore(file, dir.path()).await;
        assert!(
            matches!(refused, Err(RestoreError::NotABackup | RestoreError::Db(_))),
            "{file:?} : {refused:?}"
        );
    }
    assert!(matches!(
        db.restore(&junk, dir.path()).await,
        Err(RestoreError::NotABackup)
    ));
    assert_eq!(names(&db.users().await.unwrap()), ["alice"]);
}

#[tokio::test]
async fn a_backup_from_a_newer_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("shiori.db")).await.unwrap();
    let newer = db.backup(dir.path(), "future").await.unwrap();
    let mut conn = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&newer))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
         SELECT 9999, 'future', installed_on, 1, checksum, 0 FROM _sqlx_migrations WHERE version = 1",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();
    assert!(matches!(
        db.restore(&newer, dir.path()).await,
        Err(RestoreError::TooNew)
    ));
}

#[tokio::test]
async fn a_migration_is_preceded_by_a_backup_and_old_backups_still_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shiori.db");
    let backups = dir.path().join("backups");
    previous_version(&path).await;
    let db = Db::open_backing_up(&path, Some(&backups)).await.unwrap();
    let saved: Vec<_> = std::fs::read_dir(&backups)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saved.len(), 1);
    let name = saved[0].file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with("shiori-avant-0021-"), "{name}");

    // Base à jour : pas de nouvelle sauvegarde ; base neuve non plus.
    db.pool().close().await;
    Db::open_backing_up(&path, Some(&backups)).await.unwrap();
    Db::open_backing_up(&dir.path().join("neuve.db"), Some(&backups))
        .await
        .unwrap();
    assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);

    // La sauvegarde d'avant la migration se restaure dans une base à jour.
    let current = Db::open(&dir.path().join("autre.db")).await.unwrap();
    current.restore(&saved[0], dir.path()).await.unwrap();
    assert_eq!(names(&current.users().await.unwrap()), ["ancien"]);
}

#[tokio::test]
async fn a_backup_that_cannot_be_written_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("shiori.db")).await.unwrap();
    let file = dir.path().join("fichier");
    std::fs::write(&file, b"").unwrap();
    assert!(db.backup(&file, "shiori").await.is_err());
}
