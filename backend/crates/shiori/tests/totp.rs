//! Double authentification de bout en bout (lot 7.5) : activation, connexion en deux temps,
//! codes de secours, désactivation par le compte et par l'administrateur.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::server::{self, App};
use shiori_db::Role;
use tower::ServiceExt;

struct Fixture {
    _dir: tempfile::TempDir,
    state: Arc<App>,
    app: Router,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(shiori_db::Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let state = Arc::new(App::new(db, images));
    for (name, role) in [("admin", Role::Admin), ("bob", Role::Reader)] {
        state
            .db
            .create_user(name, "mot de passe", role)
            .await
            .unwrap();
        state
            .db
            .set_password(name, "mot de passe", false, None)
            .await
            .unwrap();
    }
    let app = server::router(state.clone(), None);
    Fixture {
        _dir: dir,
        state,
        app,
    }
}

/// Statut, cookie posé (`shiori_session=…`, vide sinon) et corps JSON (`Null` si vide).
async fn call(
    f: &Fixture,
    cookie: Option<&str>,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, String, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = f
        .app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let cookie = parts
        .headers
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = body.collect().await.unwrap().to_bytes();
    let value = match bytes.is_empty() {
        true => Value::Null,
        false => serde_json::from_slice(&bytes).unwrap(),
    };
    (parts.status, cookie, value)
}

async fn login(f: &Fixture, name: &str) -> (StatusCode, String, Value) {
    let body = json!({ "username": name, "password": "mot de passe" });
    call(f, None, "POST", "/api/auth/login", Some(body)).await
}

fn password() -> Value {
    json!({ "password": "mot de passe" })
}

/// Code TOTP juste maintenant pour ce secret (base32).
fn current_code(secret: &str) -> String {
    let secret = totp_rs::Secret::try_from_base32(secret).unwrap();
    totp_rs::Builder::new()
        .with_secret(secret)
        .build()
        .unwrap()
        .generate_current()
        .to_string()
}

/// Active la double authentification de `name` ; renvoie le cookie, le secret et les codes de secours.
async fn enable(f: &Fixture, name: &str) -> (String, String, Vec<String>) {
    let (_, cookie, _) = login(f, name).await;
    let (status, _, enrollment) =
        call(f, Some(&cookie), "POST", "/api/me/totp", Some(password())).await;
    assert_eq!(status, StatusCode::OK, "{enrollment}");
    let secret = enrollment["secret"].as_str().unwrap().to_owned();
    let confirm = json!({ "code": current_code(&secret) });
    let (status, _, codes) = call(
        f,
        Some(&cookie),
        "POST",
        "/api/me/totp/confirm",
        Some(confirm),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{codes}");
    let codes = codes["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();
    (cookie, secret, codes)
}

#[tokio::test]
async fn enrollment_gives_a_qr_then_needs_a_right_code() {
    let f = fixture().await;
    let (_, cookie, _) = login(&f, "bob").await;
    let (_, _, me) = call(&f, Some(&cookie), "GET", "/api/me", None).await;
    assert_eq!(
        (me["totp"].as_bool(), me["recovery_codes_left"].as_i64()),
        (Some(false), Some(0))
    );
    let (status, _, body) = call(
        &f,
        Some(&cookie),
        "POST",
        "/api/me/totp/confirm",
        Some(json!({ "code": "123456" })),
    )
    .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("totp_not_started"))
    );

    let (status, _, body) = call(&f, Some(&cookie), "POST", "/api/me/totp", None).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "mot de passe exigé : {body}"
    );
    let wrong = json!({ "password": "faux" });
    let (status, _, body) = call(&f, Some(&cookie), "POST", "/api/me/totp", Some(wrong)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("wrong_password"))
    );
    let (_, _, enrollment) =
        call(&f, Some(&cookie), "POST", "/api/me/totp", Some(password())).await;
    assert!(enrollment["qr_svg"].as_str().unwrap().contains("<svg"));
    assert!(
        enrollment["uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/Shiori:bob?")
    );
    let (status, _, body) = call(
        &f,
        Some(&cookie),
        "POST",
        "/api/me/totp/confirm",
        Some(json!({ "code": "000000" })),
    )
    .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_code"))
    );
    let (status, _, _) = call(
        &f,
        Some(&cookie),
        "POST",
        "/api/me/totp/confirm",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let code = current_code(enrollment["secret"].as_str().unwrap());
    let (status, _, codes) = call(
        &f,
        Some(&cookie),
        "POST",
        "/api/me/totp/confirm",
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(codes["recovery_codes"].as_array().unwrap().len(), 10);
    let (_, _, me) = call(&f, Some(&cookie), "GET", "/api/me", None).await;
    assert_eq!(
        (me["totp"].as_bool(), me["recovery_codes_left"].as_i64()),
        (Some(true), Some(10))
    );
    let (status, _, body) = call(&f, Some(&cookie), "POST", "/api/me/totp", Some(password())).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("totp_enabled"))
    );
    // L'administrateur le voit dans la liste des comptes.
    let (_, admin, _) = login(&f, "admin").await;
    let (_, _, users) = call(&f, Some(&admin), "GET", "/api/admin/users", None).await;
    let bob = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "bob")
        .unwrap();
    assert_eq!(bob["totp"], true);
}

#[tokio::test]
async fn login_asks_for_the_code_then_opens_the_session() {
    let f = fixture().await;
    let (_, _, codes) = enable(&f, "bob").await;
    let (status, cookie, body) = login(&f, "bob").await;
    assert_eq!(status, StatusCode::OK);
    assert!(cookie.is_empty(), "aucune session avant le code");
    assert_eq!(body["totp_required"], true);
    let challenge = body["challenge"].as_str().unwrap().to_owned();

    let wrong = json!({ "challenge": challenge, "code": "000000" });
    let (status, _, body) = call(&f, None, "POST", "/api/auth/totp", Some(wrong)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("invalid_code"))
    );
    let unknown = json!({ "challenge": "inventé", "code": codes[0] });
    let (status, _, body) = call(&f, None, "POST", "/api/auth/totp", Some(unknown)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("challenge_expired"))
    );

    // Un code de secours ouvre la session, une seule fois ; le défi aussi ne sert qu'une fois.
    let good = json!({ "challenge": challenge, "code": codes[0] });
    let (status, cookie, body) = call(&f, None, "POST", "/api/auth/totp", Some(good.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["username"], "bob");
    let (status, _, me) = call(&f, Some(&cookie), "GET", "/api/me", None).await;
    assert_eq!(
        (status, me["recovery_codes_left"].as_i64()),
        (StatusCode::OK, Some(9))
    );
    let (status, _, _) = call(&f, None, "POST", "/api/auth/totp", Some(good)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Application native : le jeton dans la réponse.
    let app = json!({ "username": "bob", "password": "mot de passe", "client": "app" });
    let (_, _, body) = call(&f, None, "POST", "/api/auth/login", Some(app)).await;
    let answer = json!({ "challenge": body["challenge"], "code": codes[1] });
    let (status, cookie, body) = call(&f, None, "POST", "/api/auth/totp", Some(answer)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(cookie.is_empty());
    assert!(body["token"].as_str().unwrap().starts_with("shiori_s_"));
}

#[tokio::test]
async fn wrong_codes_pause_the_account_and_a_disabled_account_gets_no_session() {
    let f = fixture().await;
    let (_, _, codes) = enable(&f, "bob").await;
    let (_, _, body) = login(&f, "bob").await;
    let challenge = body["challenge"].clone();
    let mut last = StatusCode::OK;
    for _ in 0..7 {
        let wrong = json!({ "challenge": challenge, "code": "000000" });
        last = call(&f, None, "POST", "/api/auth/totp", Some(wrong))
            .await
            .0;
    }
    assert_eq!(
        last,
        StatusCode::TOO_MANY_REQUESTS,
        "pause après les essais gratuits"
    );
    let good = json!({ "challenge": challenge, "code": codes[0] });
    let (status, _, _) = call(&f, None, "POST", "/api/auth/totp", Some(good)).await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "même avec le bon code, pendant la pause"
    );

    // Une autre instance (limite neuve) : compte désactivé entre le mot de passe et le code.
    let f = fixture().await;
    let (_, _, codes) = enable(&f, "bob").await;
    let (_, _, body) = login(&f, "bob").await;
    let bob = f.state.db.user_by_name("bob").await.unwrap().unwrap();
    let change = shiori_db::UserChange {
        disabled: Some(true),
        ..Default::default()
    };
    f.state.db.update_user(bob.id, change).await.unwrap();
    let answer = json!({ "challenge": body["challenge"], "code": codes[0] });
    let (status, cookie, body) = call(&f, None, "POST", "/api/auth/totp", Some(answer)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("unauthenticated"))
    );
    assert!(cookie.is_empty());
}

#[tokio::test]
async fn disabling_needs_the_password_and_a_code_or_the_admin() {
    let f = fixture().await;
    let (cookie, secret, _) = enable(&f, "bob").await;
    let uri = "/api/me/totp";
    let no_password = json!({ "password": "faux", "code": current_code(&secret) });
    let (status, _, body) = call(&f, Some(&cookie), "DELETE", uri, Some(no_password)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("wrong_password"))
    );
    let no_code = json!({ "password": "mot de passe", "code": "000000" });
    let (status, _, body) = call(&f, Some(&cookie), "DELETE", uri, Some(no_code)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("invalid_code"))
    );
    let (status, _, _) = call(&f, Some(&cookie), "DELETE", uri, Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, admin, _) = login(&f, "admin").await;
    let bob = f.state.db.user_by_name("bob").await.unwrap().unwrap().id;

    // L'administrateur la retire (téléphone perdu) ; ensuite le compte n'a plus rien à retirer.
    let (status, _, _) = call(
        &f,
        Some(&admin),
        "DELETE",
        &format!("/api/admin/users/{bob}/totp"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(
        &f,
        Some(&admin),
        "DELETE",
        "/api/admin/users/999/totp",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let again = json!({ "password": "mot de passe", "code": "000000" });
    let (status, _, body) = call(&f, Some(&cookie), "DELETE", uri, Some(again)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("totp_disabled"))
    );
    let (_, cookie, body) = login(&f, "bob").await;
    assert!(body.get("totp_required").is_none());
    assert!(!cookie.is_empty());

    // Par le compte lui-même, avec un code de secours.
    let (cookie, _, codes) = enable(&f, "bob").await;
    let good = json!({ "password": "mot de passe", "code": codes[2] });
    let (status, _, _) = call(&f, Some(&cookie), "DELETE", uri, Some(good)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!f.state.db.totp_enabled(bob).await.unwrap());
}
