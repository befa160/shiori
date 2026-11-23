use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use super::*;

/// Faux serveur : accepte une connexion, renvoie la requête reçue (texte brut) et répond `reply`.
fn serve(reply: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let length = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .map(|n| n.parse().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        tx.send(head + &String::from_utf8(body).unwrap()).unwrap();
        (&stream).write_all(reply.as_bytes()).unwrap();
    });
    (address, rx)
}

fn get(path: &str) -> Request<Vec<u8>> {
    Request::get(format!("https://shiori.localhost{path}"))
        .header(header::HOST, "shiori.localhost")
        .header("sec-fetch-site", "same-origin")
        .header(header::ACCEPT_LANGUAGE, "fr-FR,fr;q=0.9")
        .body(Vec::new())
        .unwrap()
}

fn body(response: &Response<Vec<u8>>) -> String {
    String::from_utf8(response.body().clone()).unwrap()
}

#[test]
fn the_address_is_asked_checked_saved_then_kept() {
    let dir = tempfile::tempdir().unwrap();
    let relay = Relay::new(dir.path().to_owned());
    // Pas encore de serveur : la page de réglage, quelle que soit l'adresse demandée.
    let response = relay.handle(get("/series/3"));
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body(&response).contains("Adresse du serveur Shiori"));
    // Adresse refusée : message, saisie gardée (échappée).
    let response = relay.handle(get("/__shiori/setup?server=ftp%3A%2F%2Fnas%3Cb%3E"));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(body(&response).contains("Adresse non valable"));
    assert!(body(&response).contains("ftp://nas&lt;b&gt;"));
    // Adresse acceptée : enregistrée, puis l'interface.
    let response = relay.handle(get(
        "/__shiori/setup?server=+http%3A%2F%2F192.168.1.20%3A7878%2F+",
    ));
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("server.txt")).unwrap(),
        "http://192.168.1.20:7878"
    );
    // Gardée au prochain lancement ; la page de réglage la montre pour la changer (en anglais
    // si l'appareil n'est pas en français).
    let relay = Relay::new(dir.path().to_owned());
    let mut request = get("/__shiori/setup");
    request.headers_mut().remove(header::ACCEPT_LANGUAGE);
    let page = body(&relay.handle(request));
    assert!(page.contains("Shiori server address"));
    assert!(page.contains(r#"value="http://192.168.1.20:7878""#));
    // Pas de service worker dans l'application.
    assert_eq!(relay.handle(get("/sw.js")).status(), StatusCode::NOT_FOUND);
}

#[test]
fn requests_go_to_the_server_as_they_are_and_come_back_as_they_are() {
    let (server, received) = serve(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: br\r\nSet-Cookie: shiori_session=abc; Path=/; HttpOnly\r\nContent-Length: 4\r\nConnection: close\r\n\r\n\x0b\x01\x7f{",
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("server.txt"), &server).unwrap();
    let relay = Relay::new(dir.path().to_owned());
    let request = Request::post("https://shiori.localhost/api/auth/login?next=%2F")
        .header(header::HOST, "shiori.localhost")
        .header(header::COOKIE, "shiori_session=old")
        .header(header::CONTENT_TYPE, "application/json")
        .header("sec-fetch-site", "same-origin")
        .body(br#"{"username":"a"}"#.to_vec())
        .unwrap();
    let response = relay.handle(request);
    let sent = received.recv().unwrap();
    assert!(
        sent.starts_with("POST /api/auth/login?next=%2F HTTP/1.1\r\n"),
        "{sent}"
    );
    let host = server.strip_prefix("http://").unwrap();
    assert!(
        sent.to_ascii_lowercase()
            .contains(&format!("host: {host}\r\n")),
        "{sent}"
    );
    assert!(sent.contains("cookie: shiori_session=old\r\n"), "{sent}");
    assert!(sent.contains("sec-fetch-site: same-origin\r\n"), "{sent}");
    assert!(sent.ends_with(r#"{"username":"a"}"#), "{sent}");
    // Réponse intacte : statut, cookie, corps compressé non décodé.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::SET_COOKIE],
        "shiori_session=abc; Path=/; HttpOnly"
    );
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "br");
    assert!(!response.headers().contains_key(header::CONNECTION));
    assert_eq!(response.body(), b"\x0b\x01\x7f{");
}

#[test]
fn a_get_has_no_body_and_a_redirect_is_not_followed() {
    let (server, received) = serve(
        "HTTP/1.1 302 Found\r\nLocation: /login\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("server.txt"), &server).unwrap();
    let response = Relay::new(dir.path().to_owned()).handle(get("/admin"));
    let sent = received.recv().unwrap().to_ascii_lowercase();
    assert!(sent.starts_with("get /admin http/1.1\r\n"), "{sent}");
    assert!(!sent.contains("content-length"), "{sent}");
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers()[header::LOCATION], "/login");
}

#[test]
fn an_unreachable_server_says_so_and_offers_to_change_the_address() {
    // Port libéré tout de suite : rien n'y répond.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("server.txt"),
        format!("http://127.0.0.1:{port}"),
    )
    .unwrap();
    let response = Relay::new(dir.path().to_owned()).handle(get("/"));
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let page = body(&response);
    assert!(page.contains("Le serveur ne répond pas"));
    assert!(page.contains(&format!(r#"value="http://127.0.0.1:{port}""#)));
}

#[test]
fn an_address_that_cannot_be_saved_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    // Le dossier n'existe pas : l'écriture échoue.
    let relay = Relay::new(dir.path().join("absent"));
    let response = relay.handle(get(
        "/__shiori/setup?server=https%3A%2F%2Fshiori.example.com",
    ));
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body(&response).contains("pas pu être enregistrée"));
}

#[test]
fn only_a_bare_http_or_https_address_is_a_server() {
    assert_eq!(
        server_address("https://shiori.example.com/").as_deref(),
        Some("https://shiori.example.com")
    );
    assert_eq!(
        server_address("http://[::1]:7878").as_deref(),
        Some("http://[::1]:7878")
    );
    for bad in [
        "shiori.local",
        "ftp://nas",
        "http://nas/shiori",
        "http://nas?x=1",
        "http://nas#a",
        "http://",
        "",
    ] {
        assert_eq!(server_address(bad), None, "{bad}");
    }
}

#[test]
fn form_values_are_decoded() {
    assert_eq!(form_decode("a+b%2Fc%3A"), "a b/c:");
    // `%` sans deux chiffres hexadécimaux : gardé tel quel.
    assert_eq!(form_decode("100%"), "100%");
    assert_eq!(form_decode("%zz%4"), "%zz%4");
}

#[test]
fn a_foreign_page_cannot_change_the_server() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("server.txt"), "http://nas:7878").unwrap();
    let relay = Relay::new(dir.path().to_owned());
    for site in [Some("cross-site"), Some("same-site"), None] {
        let mut request = get("/__shiori/setup?server=https%3A%2F%2Fpirate.example");
        match site {
            Some(site) => request
                .headers_mut()
                .insert("sec-fetch-site", HeaderValue::from_static(site)),
            None => request.headers_mut().remove("sec-fetch-site"),
        };
        let response = relay.handle(request);
        assert_eq!(response.status(), StatusCode::OK, "{site:?}");
        assert!(
            body(&response).contains(r#"value="http://nas:7878""#),
            "{site:?}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.path().join("server.txt")).unwrap(),
        "http://nas:7878"
    );
    // Adresse tapée par l'utilisateur : acceptée.
    let mut request = get("/__shiori/setup?server=https%3A%2F%2Fshiori.example.com");
    request
        .headers_mut()
        .insert("sec-fetch-site", HeaderValue::from_static("none"));
    assert_eq!(relay.handle(request).status(), StatusCode::SEE_OTHER);
}
