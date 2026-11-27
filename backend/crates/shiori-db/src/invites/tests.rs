use super::*;
use crate::{LibraryKind, UserChange};

async fn db() -> (tempfile::TempDir, Db, i64) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let admin = db
        .create_user("admin", "mot de passe", Role::Admin)
        .await
        .unwrap()
        .id;
    (dir, db, admin)
}

async fn library(db: &Db, dir: &std::path::Path, name: &str) -> i64 {
    let root = dir.join(name);
    std::fs::create_dir(&root).unwrap();
    db.create_library(name, &root, LibraryKind::Manga)
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn an_invite_creates_a_reader_with_its_libraries_and_works_once() {
    let (dir, db, admin) = db().await;
    library(&db, dir.path(), "Mangas").await;
    let bd = library(&db, dir.path(), "BD").await;
    let new = db
        .create_invite(admin, Role::Reader, Some(vec![bd, 999]), 7)
        .await
        .unwrap();
    assert_eq!(new.token.len(), 64);
    assert_eq!(new.invite.libraries, [bd]);
    assert!(new.invite.expires_at > new.invite.created_at);
    assert_eq!(
        db.invites().await.unwrap(),
        std::slice::from_ref(&new.invite)
    );
    assert_eq!(
        db.invite_role(&new.token).await.unwrap(),
        Some(Role::Reader)
    );

    let user = db
        .accept_invite(&new.token, "Élise", "mot de passe choisi")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.role, Role::Reader);
    assert!(!user.must_change_password, "mot de passe choisi, définitif");
    assert_eq!(db.user_libraries(user.id).await.unwrap(), [bd]);
    assert!(
        db.check_password("élise", "mot de passe choisi")
            .await
            .unwrap()
            .is_some()
    );

    // Usage unique : le lien ne vaut plus rien et sort de la liste.
    assert_eq!(db.invite_role(&new.token).await.unwrap(), None);
    assert!(
        db.accept_invite(&new.token, "autre", "mot de passe choisi")
            .await
            .unwrap()
            .is_none()
    );
    assert!(db.invites().await.unwrap().is_empty());
    assert!(!db.revoke_invite(new.invite.id).await.unwrap());
}

#[tokio::test]
async fn without_a_list_a_reader_gets_todays_libraries_and_an_admin_none() {
    let (dir, db, admin) = db().await;
    let mangas = library(&db, dir.path(), "Mangas").await;
    let reader = db
        .create_invite(admin, Role::Reader, None, 1)
        .await
        .unwrap();
    assert_eq!(reader.invite.libraries, [mangas]);
    // Une bibliothèque créée après l'invitation reste fermée, comme pour un compte existant.
    library(&db, dir.path(), "BD").await;
    let user = db
        .accept_invite(&reader.token, "lecteur", "mot de passe choisi")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(db.user_libraries(user.id).await.unwrap(), [mangas]);

    let admin_invite = db
        .create_invite(admin, Role::Admin, Some(vec![mangas]), 30)
        .await
        .unwrap();
    assert!(admin_invite.invite.libraries.is_empty());
    let other = db
        .accept_invite(&admin_invite.token, "chef", "mot de passe choisi")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(other.role, Role::Admin);
}

#[tokio::test]
async fn an_expired_or_revoked_invite_is_refused_and_a_bad_account_keeps_it() {
    let (_dir, db, admin) = db().await;
    let expired = db
        .create_invite(admin, Role::Reader, None, 7)
        .await
        .unwrap();
    sqlx::query("UPDATE invites SET expires_at = '2000-01-01T00:00:00.000Z' WHERE id = ?")
        .bind(expired.invite.id)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(db.invite_role(&expired.token).await.unwrap(), None);
    assert!(
        db.accept_invite(&expired.token, "élise", "mot de passe choisi")
            .await
            .unwrap()
            .is_none()
    );
    // Toujours listée (périmée), pour que l'administrateur la voie et la supprime.
    assert_eq!(db.invites().await.unwrap().len(), 1);

    let kept = db
        .create_invite(admin, Role::Reader, None, 7)
        .await
        .unwrap();
    // Nom pris, nom invalide, mot de passe trop court : rien n'est consommé.
    assert!(matches!(
        db.accept_invite(&kept.token, "ADMIN", "mot de passe choisi")
            .await,
        Err(AccountError::NameTaken(_))
    ));
    assert!(matches!(
        db.accept_invite(&kept.token, "a b", "mot de passe choisi")
            .await,
        Err(AccountError::InvalidName)
    ));
    assert!(matches!(
        db.accept_invite(&kept.token, "élise", "court").await,
        Err(AccountError::WeakPassword)
    ));
    assert_eq!(
        db.invite_role(&kept.token).await.unwrap(),
        Some(Role::Reader)
    );

    assert!(db.revoke_invite(kept.invite.id).await.unwrap());
    assert_eq!(db.invite_role(&kept.token).await.unwrap(), None);
    assert!(
        db.accept_invite("inconnu", "élise", "mot de passe choisi")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn the_invite_is_claimed_inside_the_transaction() {
    // Le lien devient invalide entre la vérification et la transaction (accepté ailleurs).
    let (_dir, db, admin) = db().await;
    let new = db
        .create_invite(admin, Role::Reader, None, 7)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        db.accept_invite(&new.token, "un", "mot de passe choisi"),
        db.accept_invite(&new.token, "deux", "mot de passe choisi"),
    );
    let created = [a.unwrap(), b.unwrap()];
    assert_eq!(created.iter().filter(|u| u.is_some()).count(), 1);
}

#[tokio::test]
async fn an_invite_dies_with_its_authors_admin_rights() {
    let (_dir, db, admin) = db().await;
    let other = db
        .create_user("autre", "mot de passe", Role::Admin)
        .await
        .unwrap()
        .id;
    let kept = db
        .create_invite(admin, Role::Admin, None, 30)
        .await
        .unwrap();
    db.update_user(
        admin,
        UserChange {
            role: Some(Role::Reader),
            ..UserChange::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(db.invite_role(&kept.token).await.unwrap(), None);
    assert!(db.invites().await.unwrap().is_empty());
    assert!(
        db.accept_invite(&kept.token, "revenant", "mot de passe choisi")
            .await
            .unwrap()
            .is_none()
    );
    // Auteur supprimé : plus d'auteur, plus d'invitation.
    let orphan = db
        .create_invite(other, Role::Reader, None, 7)
        .await
        .unwrap();
    db.create_user("troisieme", "mot de passe", Role::Admin)
        .await
        .unwrap();
    assert!(db.delete_user(other).await.unwrap());
    assert_eq!(db.invite_role(&orphan.token).await.unwrap(), None);
}
