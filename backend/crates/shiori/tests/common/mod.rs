//! Aides partagées par les tests d'intégration du serveur.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, header};
use shiori::server::{self, App};
use shiori_db::Role;

/// Routeur dont chaque requête porte la session d'un administrateur, sauf si le test donne
/// lui-même un cookie.
pub async fn router(state: Arc<App>) -> Router {
    let admin = match state.db.user_by_name("admin").await.unwrap() {
        Some(admin) => admin,
        None => {
            state
                .db
                .create_user("admin", "mot de passe", Role::Admin)
                .await
                .unwrap();
            // Mot de passe définitif : un provisoire bloquerait toutes les routes.
            state
                .db
                .set_password("admin", "mot de passe", false, None)
                .await
                .unwrap();
            state.db.user_by_name("admin").await.unwrap().unwrap()
        }
    };
    let session = state.db.create_session(admin.id, "tests").await.unwrap();
    let cookie = HeaderValue::from_str(&format!("shiori_session={}", session.token)).unwrap();
    server::router(state, None).layer(axum::middleware::map_request(
        move |mut request: Request| {
            let cookie = cookie.clone();
            async move {
                request
                    .headers_mut()
                    .entry(header::COOKIE)
                    .or_insert(cookie);
                request
            }
        },
    ))
}
