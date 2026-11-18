use super::*;
use crate::Role;

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

/// Code juste à cet instant pour ce secret.
fn code(secret: &str, time: u64) -> String {
    totp(secret, "x").unwrap().generate(time).to_string()
}

const T: u64 = 1_800_000_000;

#[tokio::test]
async fn enrollment_is_inactive_until_a_right_code_then_gives_ten_recovery_codes() {
    let (_dir, db, user) = db().await;
    let first = db.start_totp(user, "élise").await.unwrap();
    assert!(
        first.uri.starts_with("otpauth://totp/Shiori:"),
        "{}",
        first.uri
    );
    assert!(first.uri.contains(&first.secret));
    // Recommencer remplace le secret d'un enrôlement non confirmé.
    let enrollment = db.start_totp(user, "élise").await.unwrap();
    assert_ne!(first.secret, enrollment.secret);
    assert!(!db.totp_enabled(user).await.unwrap());
    // Pas encore active : un code ne vaut rien à la connexion.
    assert!(
        !db.check_second_factor_at(user, &code(&enrollment.secret, T), T)
            .await
            .unwrap()
    );

    assert!(matches!(
        db.confirm_totp_at(user, "000000", T).await,
        Err(TotpError::WrongCode)
    ));
    assert!(matches!(
        db.confirm_totp_at(user, &code(&first.secret, T), T).await,
        Err(TotpError::WrongCode)
    ));
    let codes = db
        .confirm_totp_at(user, &code(&enrollment.secret, T), T)
        .await
        .unwrap();
    assert_eq!(codes.len(), 10);
    assert!(
        codes
            .iter()
            .all(|c| c.len() == 14 && c.matches('-').count() == 2)
    );
    assert!(db.totp_enabled(user).await.unwrap());
    assert_eq!(db.recovery_codes_left(user).await.unwrap(), 10);
    assert!(matches!(
        db.start_totp(user, "élise").await,
        Err(TotpError::AlreadyEnabled)
    ));
    assert!(matches!(
        db.confirm_totp_at(user, "123456", T).await,
        Err(TotpError::NotStarted)
    ));
}

#[tokio::test]
async fn a_code_is_accepted_once_within_one_step_of_tolerance() {
    let (_dir, db, user) = db().await;
    let secret = db.start_totp(user, "élise").await.unwrap().secret;
    db.confirm_totp_at(user, &code(&secret, T), T)
        .await
        .unwrap();
    // Le code de l'enrôlement ne resert pas.
    assert!(
        !db.check_second_factor_at(user, &code(&secret, T), T)
            .await
            .unwrap()
    );
    // Pas suivant, saisi avec des espaces ; puis rejoué : refusé.
    let next = code(&secret, T + 30);
    let spaced = format!("{} {}", &next[..3], &next[3..]);
    assert!(
        db.check_second_factor_at(user, &spaced, T + 30)
            .await
            .unwrap()
    );
    assert!(
        !db.check_second_factor_at(user, &next, T + 30)
            .await
            .unwrap()
    );
    // Tolérance d'un pas (horloge du téléphone en retard), pas de deux.
    assert!(
        db.check_second_factor_at(user, &code(&secret, T + 60), T + 90)
            .await
            .unwrap()
    );
    assert!(
        !db.check_second_factor_at(user, &code(&secret, T + 120), T + 300)
            .await
            .unwrap()
    );
    assert!(
        !db.check_second_factor_at(user, "12345a", T + 300)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_recovery_code_works_once_whatever_its_case_and_dashes() {
    let (_dir, db, user) = db().await;
    let secret = db.start_totp(user, "élise").await.unwrap().secret;
    let codes = db
        .confirm_totp_at(user, &code(&secret, T), T)
        .await
        .unwrap();
    let typed = codes[3].replace('-', "").to_uppercase();
    assert!(db.check_second_factor_at(user, &typed, T).await.unwrap());
    assert!(!db.check_second_factor_at(user, &codes[3], T).await.unwrap());
    assert!(
        !db.check_second_factor_at(user, "abcd-efgh-ijkm", T)
            .await
            .unwrap()
    );
    assert_eq!(db.recovery_codes_left(user).await.unwrap(), 9);
    // Nouvel enrôlement après désactivation : les anciens codes ne valent plus rien.
    assert!(db.disable_totp(user).await.unwrap());
    assert!(!db.disable_totp(user).await.unwrap());
    assert!(!db.totp_enabled(user).await.unwrap());
    assert!(!db.check_second_factor_at(user, &codes[4], T).await.unwrap());
    let secret = db.start_totp(user, "élise").await.unwrap().secret;
    db.confirm_totp_at(user, &code(&secret, T), T)
        .await
        .unwrap();
    assert!(!db.check_second_factor_at(user, &codes[5], T).await.unwrap());
}

#[tokio::test]
async fn a_damaged_secret_is_an_error_not_a_pass() {
    let (_dir, db, user) = db().await;
    sqlx::query("INSERT INTO user_totp (user_id, secret, enabled_at) VALUES (?, '!!!', 'x')")
        .bind(user)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(db.check_second_factor_at(user, "123456", T).await.is_err());
    // Le code public lit l'horloge du système.
    assert!(db.check_second_factor(user, "123456").await.is_err());
    assert_eq!(normalize(" Ab-C d "), "abcd");
}

#[tokio::test]
async fn deleting_the_account_removes_its_second_factor() {
    let (_dir, db, user) = db().await;
    let secret = db.start_totp(user, "élise").await.unwrap().secret;
    db.confirm_totp(user, &code(&secret, now())).await.unwrap();
    // Un administrateur reste : la suppression n'est pas refusée pour lui.
    db.create_user("admin", "mot de passe", Role::Admin)
        .await
        .unwrap();
    assert!(db.delete_user(user).await.unwrap());
    let left: i64 = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM user_totp) + (SELECT COUNT(*) FROM user_recovery_codes) AS n",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
    .get("n");
    assert_eq!(left, 0);
}
