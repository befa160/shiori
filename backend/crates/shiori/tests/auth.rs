//! Installation, connexion et sessions de bout en bout (lot 7.3).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::server::{self, App};
use shiori_db::Role;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
    let app = server::router(state.clone(), None);
    Fixture {
        _dir: dir,
        state,
        app,
    }
}

/// Réponse : statut, en-têtes, corps JSON (`Null` si vide).
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let value = match bytes.is_empty() {
        true => Value::Null,
        false => serde_json::from_slice(&bytes).unwrap(),
    };
    (parts.status, parts.headers, value)
}

fn set_cookie(headers: &HeaderMap) -> &str {
    headers.get(header::SET_COOKIE).unwrap().to_str().unwrap()
}

/// `shiori_session=<jeton>` tiré d'un `Set-Cookie`, prêt à renvoyer.
fn cookie_of(headers: &HeaderMap) -> String {
    set_cookie(headers).split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn the_api_is_closed_without_a_session_but_the_interface_and_health_stay_open() {
    let f = fixture().await;
    for uri in ["/api/libraries", "/api/admin/scans", "/api/nope"] {
        let (status, _, body) = call(&f.app, "GET", uri, None, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(body["code"], "unauthenticated");
    }
    let (status, _, _) = call(&f.app, "POST", "/api/libraries", Some(json!({})), &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = call(&f.app, "GET", "/api/health", None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    // L'interface (hors /api) est servie : c'est elle qui montre la page de connexion.
    let (status, _, _) = call(&f.app, "GET", "/api", None, &[]).await;
    assert_ne!(status, StatusCode::UNAUTHORIZED);
    let wrong = [("cookie", "shiori_session=shiori_s_faux")];
    let (status, _, _) = call(&f.app, "GET", "/api/libraries", None, &wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn setup_needs_the_token_creates_the_admin_once_and_signs_in() {
    let f = fixture().await;
    let (_, _, state) = call(&f.app, "GET", "/api/auth/state", None, &[]).await;
    assert_eq!(state, json!({ "setup_required": true, "user": null }));

    let mut setup = json!({ "token": "mauvais", "username": "alice", "password": "mot de passe" });
    let (status, _, body) = call(&f.app, "POST", "/api/auth/setup", Some(setup.clone()), &[]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("invalid_setup_token"))
    );

    setup["token"] = json!(format!(" {} ", f.state.setup_token()));
    setup["password"] = json!("court");
    let (status, _, body) = call(&f.app, "POST", "/api/auth/setup", Some(setup.clone()), &[]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("weak_password"))
    );

    setup["password"] = json!("mot de passe");
    let (status, headers, body) =
        call(&f.app, "POST", "/api/auth/setup", Some(setup.clone()), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["username"], "alice");
    assert_eq!(body["user"]["role"], "admin");
    assert_eq!(body["user"]["must_change_password"], false);
    assert!(
        body.get("token").is_none(),
        "un navigateur n'a que le cookie : {body}"
    );
    let cookie = set_cookie(&headers);
    assert!(cookie.starts_with("shiori_session=shiori_s_"), "{cookie}");
    assert!(
        cookie.ends_with("; Path=/; HttpOnly; SameSite=Lax; Max-Age=7776000"),
        "{cookie}"
    );

    let session = cookie_of(&headers);
    let (_, _, state) = call(
        &f.app,
        "GET",
        "/api/auth/state",
        None,
        &[("cookie", &session)],
    )
    .await;
    assert_eq!(state["setup_required"], false);
    assert_eq!(state["user"]["username"], "alice");
    let (status, _, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("cookie", &session)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    setup["username"] = json!("mallory");
    let (status, _, body) = call(&f.app, "POST", "/api/auth/setup", Some(setup), &[]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("already_set_up"))
    );
}

#[tokio::test]
async fn login_gives_a_cookie_or_an_app_token_and_logout_closes_it() {
    let f = fixture().await;
    f.state
        .db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let bad = json!({ "username": "bob", "password": "faux" });
    let (status, headers, body) = call(&f.app, "POST", "/api/auth/login", Some(bad), &[]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("invalid_credentials"))
    );
    assert!(headers.get(header::SET_COOKIE).is_none());
    let unknown = json!({ "username": "inconnu", "password": "mot de passe" });
    let (_, _, same) = call(&f.app, "POST", "/api/auth/login", Some(unknown), &[]).await;
    assert_eq!(same, body, "même réponse pour un nom inconnu");
    let (status, _, _) = call(&f.app, "POST", "/api/auth/login", Some(json!({})), &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Navigateur, derrière un proxy HTTPS : cookie `Secure`.
    let good = json!({ "username": "Bob", "password": "mot de passe" });
    let proxy = [
        ("x-forwarded-proto", "https"),
        ("user-agent", "Firefox sur Android"),
    ];
    let (status, headers, body) = call(
        &f.app,
        "POST",
        "/api/auth/login",
        Some(good.clone()),
        &proxy,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["must_change_password"], true);
    assert!(set_cookie(&headers).ends_with("; Secure"));
    let browser = cookie_of(&headers);

    // Application : le jeton dans la réponse, présenté en `Bearer`, aucun cookie.
    let mut app_login = good;
    app_login["client"] = json!("app");
    let (_, headers, body) = call(&f.app, "POST", "/api/auth/login", Some(app_login), &[]).await;
    assert!(headers.get(header::SET_COOKIE).is_none());
    let token = body["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("shiori_s_"));
    let bearer = format!("Bearer {token}");
    let (status, _, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("authorization", &bearer)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let labels: Vec<String> = sqlx::query_scalar("SELECT label FROM sessions ORDER BY id")
        .fetch_all(f.state.db.pool())
        .await
        .unwrap();
    assert_eq!(labels, ["Firefox sur Android", "appareil inconnu"]);

    let (status, headers, _) = call(
        &f.app,
        "POST",
        "/api/auth/logout",
        None,
        &[("cookie", &browser)],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(set_cookie(&headers).contains("Max-Age=0"));
    let (status, _, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("cookie", &browser)],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // L'autre appareil reste connecté ; se déconnecter sans session ne fait rien de mal.
    let (status, _, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("authorization", &bearer)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&f.app, "POST", "/api/auth/logout", None, &[]).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn repeated_failures_pause_the_name_without_blocking_others() {
    let f = fixture().await;
    for name in ["bob", "alice"] {
        f.state
            .db
            .create_user(name, "mot de passe", Role::Reader)
            .await
            .unwrap();
    }
    let bad = json!({ "username": "bob", "password": "faux" });
    for _ in 0..6 {
        let (status, _, _) = call(&f.app, "POST", "/api/auth/login", Some(bad.clone()), &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    // Même le bon mot de passe attend la fin de la pause.
    let good = json!({ "username": "bob", "password": "mot de passe" });
    let (status, headers, body) = call(&f.app, "POST", "/api/auth/login", Some(good), &[]).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
    let wait: u64 = headers[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=3).contains(&wait), "{wait}");
    let other = json!({ "username": "alice", "password": "mot de passe" });
    let (status, _, _) = call(&f.app, "POST", "/api/auth/login", Some(other), &[]).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_session_used_after_an_hour_renews_its_cookie() {
    let f = fixture().await;
    let bob = f
        .state
        .db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let session = f.state.db.create_session(bob.id, "tests").await.unwrap();
    let cookie = format!("shiori_session={}", session.token);
    let (_, headers, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("cookie", &cookie)],
    )
    .await;
    assert!(
        headers.get(header::SET_COOKIE).is_none(),
        "session toute neuve"
    );
    sqlx::query("UPDATE sessions SET last_seen_at = '2000-01-01T00:00:00.000Z'")
        .execute(f.state.db.pool())
        .await
        .unwrap();
    let (_, headers, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("cookie", &cookie)],
    )
    .await;
    assert_eq!(
        set_cookie(&headers),
        format!("{cookie}; Path=/; HttpOnly; SameSite=Lax; Max-Age=7776000")
    );
    // En `Bearer`, l'application garde son jeton : pas de cookie.
    sqlx::query("UPDATE sessions SET last_seen_at = '2000-01-01T00:00:00.000Z'")
        .execute(f.state.db.pool())
        .await
        .unwrap();
    let bearer = format!("Bearer {}", session.token);
    let (_, headers, _) = call(
        &f.app,
        "GET",
        "/api/libraries",
        None,
        &[("authorization", &bearer)],
    )
    .await;
    assert!(headers.get(header::SET_COOKIE).is_none());
}

async fn serve(f: &Fixture) -> (std::net::SocketAddr, tokio::sync::oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(server::serve(
        listener,
        f.state.clone(),
        Vec::new(),
        async {
            let _ = stopped.await;
        },
    ));
    (addr, stop)
}

/// Sur une vraie connexion, l'adresse du client sert à la limite d'essais.
#[tokio::test]
async fn login_works_over_tcp() {
    let f = fixture().await;
    f.state
        .db
        .create_user("bob", "mot de passe", Role::Reader)
        .await
        .unwrap();
    let (addr, _stop) = serve(&f).await;
    let body = r#"{"username":"bob","password":"mot de passe"}"#;
    let request = format!(
        "POST /api/auth/login HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(
        response.contains("set-cookie: shiori_session=shiori_s_"),
        "{response}"
    );
}

/// Un client qui n'achève pas ses en-têtes est déconnecté (attaque « slowloris »).
#[tokio::test]
async fn a_slow_client_is_cut_off() {
    let f = fixture().await;
    let (addr, _stop) = serve(&f).await;
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: local")
        .await
        .unwrap();
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut rest)).await;
    assert!(read.is_ok(), "connexion toujours ouverte après 20 s");
}
