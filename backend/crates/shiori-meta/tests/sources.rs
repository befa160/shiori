//! Clients AniList et MangaDex contre un faux serveur local qui rejoue des réponses de la forme
//! des vraies (relevées le 2026-11-13, textes remplacés) : aucun appel réseau dans les tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use shiori_meta::{Anilist, Author, FetchError, Mangadex, Source, SourceId, Text, WebMetadata};

/// Faux serveur : une réponse par connexion, dans l'ordre ; garde chaque requête reçue.
struct Mock {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

fn mock(responses: Vec<(u16, &'static str, String)>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        for (status, extra, body) in responses {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
                head.push_str(&line);
            }
            let mut sent = vec![0; length];
            reader.read_exact(&mut sent).unwrap();
            head.push_str(&String::from_utf8_lossy(&sent));
            seen.lock().unwrap().push(head);
            let response = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}",
                body.len()
            );
            reader.get_mut().write_all(response.as_bytes()).unwrap();
        }
    });
    Mock { url, requests }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn text(lang: Option<&str>, text: &str) -> Text {
    Text {
        lang: lang.map(Into::into),
        text: text.into(),
    }
}

fn author(name: &str, story: bool, art: bool) -> Author {
    Author {
        name: name.into(),
        story,
        art,
    }
}

/// Ce qui part sur le réseau : la méthode, le chemin, l'agent générique, le titre cherché ;
/// jamais un chemin de fichier, un nom de dossier ou un nom d'utilisateur.
fn assert_discreet(request: &str, query: &str) {
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("user-agent: shiori/"), "{request}");
    for leak in ["cookie", "authorization", "referer", "/home/", "\\users\\"] {
        assert!(!lower.contains(leak), "{leak} dans {request}");
    }
    assert!(request.contains(query), "{request}");
}

#[test]
fn anilist_search_is_normalized() {
    let server = mock(vec![(200, "", fixture("anilist_search.json"))]);
    let found = Anilist::new(&server.url)
        .search("one piece", false)
        .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0],
        WebMetadata {
            id: "30013".into(),
            titles: vec![
                text(Some("ja-ro"), "ONE PIECE"),
                text(Some("en"), "One Piece"),
                text(None, "ワンピース")
            ],
            summaries: vec![text(
                Some("en"),
                "Un garçon au chapeau de paille & son équipage.\n\n(Source: exemple)"
            )],
            authors: vec![author("Eiichirou Oda", true, true)],
            genres: vec!["Action".into(), "Adventure".into()],
            tags: vec!["Pirates".into()],
            year: Some(1997),
            status: Some("ongoing".into()),
            links: vec!["https://anilist.co/manga/30013".into()],
            cover_url: Some(
                "https://s4.anilist.co/file/anilistcdn/media/manga/cover/medium/bx30013.jpg".into()
            ),
        }
    );
    // Pas encore paru : pas de statut ; scénario et dessin séparés.
    assert_eq!((found[1].status.as_deref(), found[1].year), (None, None));
    assert_eq!(
        found[1].authors,
        [
            author("Auteur A", true, false),
            author("Dessinateur B", false, true),
            // L'auteur de l'œuvre adaptée compte ; le traducteur, non.
            author("Romancier C", true, false)
        ]
    );
    let request = &server.requests.lock().unwrap()[0];
    assert!(request.starts_with("POST / "), "{request}");
    assert!(request.contains("format_not: NOVEL"));
    // Fiches pour adultes exclues par défaut.
    assert!(request.contains("isAdult: false"), "{request}");
    assert_discreet(request, r#""q":"one piece""#);
}

#[test]
fn anilist_fetches_one_entry_and_reports_errors() {
    let entry = r#"{"data":{"Media":{"id":7,"title":{"romaji":"Akira","english":null,"native":"アキラ"},"status":"FINISHED"}}}"#;
    let server = mock(vec![
        (200, "", entry.into()),
        (
            404,
            "",
            r#"{"data":{"Media":null},"errors":[{"message":"Not Found.","status":404}]}"#.into(),
        ),
        (200, "", r#"{"data":{"Media":null}}"#.into()),
        (200, "", "pas du JSON".into()),
        (200, "", r#"{"errors":[{"message":"x"}]}"#.into()),
        (500, "", String::new()),
    ]);
    let anilist = Anilist::new(&server.url);
    let akira = anilist.fetch("7").unwrap();
    assert_eq!(
        (akira.id.as_str(), akira.status.as_deref()),
        ("7", Some("completed"))
    );
    assert_eq!(
        akira.titles,
        [text(Some("ja-ro"), "Akira"), text(Some("ja"), "アキラ")]
    );
    assert!(server.requests.lock().unwrap()[0].contains(r#""id":7"#));
    assert_eq!(anilist.fetch("8"), Err(FetchError::NotFound));
    assert_eq!(anilist.fetch("9"), Err(FetchError::NotFound));
    assert!(matches!(
        anilist.fetch("10"),
        Err(FetchError::BadResponse(_))
    ));
    assert!(matches!(
        anilist.fetch("11"),
        Err(FetchError::BadResponse(_))
    ));
    assert_eq!(anilist.fetch("12"), Err(FetchError::Http(500)));
    // Un identifiant qui n'est pas un nombre ne part pas.
    assert_eq!(anilist.fetch("../x"), Err(FetchError::NotFound));
    assert_eq!(server.requests.lock().unwrap().len(), 6);
}

#[test]
fn a_429_is_honoured_without_more_requests() {
    let server = mock(vec![(
        429,
        "retry-after: 30\r\n",
        r#"{"errors":[]}"#.into(),
    )]);
    let mangadex = Mangadex::new(&server.url);
    assert_eq!(
        mangadex.search("akira", false),
        Err(FetchError::RateLimited(Duration::from_secs(30)))
    );
    // Pendant l'attente demandée, rien ne part : la requête suivante renonce aussitôt.
    assert!(matches!(
        mangadex.search("akira", false),
        Err(FetchError::RateLimited(_))
    ));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    // Sans `Retry-After`, une minute.
    let server = mock(vec![(429, "", String::new())]);
    assert_eq!(
        Anilist::new(&server.url).search("x", false),
        Err(FetchError::RateLimited(Duration::from_secs(60)))
    );
}

#[test]
fn an_unreachable_source_is_offline() {
    // Port libéré aussitôt réservé : personne n'écoute.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}");
    assert!(matches!(
        Mangadex::new(&url).search("x", false),
        Err(FetchError::Offline(_))
    ));
    assert!(matches!(
        Anilist::new(&url).fetch("1"),
        Err(FetchError::Offline(_))
    ));
}

#[test]
fn mangadex_search_and_fetch_are_normalized() {
    let one = fixture("mangadex_search.json")
        .replace(
            r#""response":"collection","data":["#,
            r#""response":"entity","data":"#,
        )
        .replace(r#"],"limit":10,"offset":0,"total":1"#, "");
    let server = mock(vec![
        (200, "", fixture("mangadex_search.json")),
        (200, "", one),
    ]);
    let mangadex = Mangadex::new(&format!("{}/", server.url));
    assert_eq!(mangadex.id(), SourceId::Mangadex);
    let found = mangadex.search("one piece", false).unwrap();
    let id = "a1c7c817-4e59-43b7-9365-09675a149a6f";
    let expected = WebMetadata {
        id: id.into(),
        // Titre principal d'abord ; « One Piece » en français n'est pas répété.
        titles: vec![
            text(Some("ja-ro"), "One Piece"),
            text(Some("ja"), "ワンピース"),
            text(Some("en"), "Wan Pīsu"),
        ],
        summaries: vec![
            text(Some("en"), "A boy and his crew."),
            text(
                Some("fr"),
                "Un garçon et son équipage.\n\nSite officiel · [lien cassé",
            ),
        ],
        authors: vec![author("Oda Eiichirou", true, true)],
        genres: vec!["Action".into()],
        tags: vec!["Monsters".into()],
        year: Some(1997),
        status: Some("ongoing".into()),
        links: vec![format!("https://mangadex.org/title/{id}")],
        cover_url: Some(format!(
            "https://uploads.mangadex.org/covers/{id}/2f4aca53.png.512.jpg"
        )),
    };
    assert_eq!(found, std::slice::from_ref(&expected));
    assert_eq!(mangadex.fetch(id).unwrap(), expected);
    let requests = server.requests.lock().unwrap();
    assert!(
        requests[0].starts_with("GET /manga?title=one+piece&")
            || requests[0].starts_with("GET /manga?title=one%20piece&"),
        "{}",
        requests[0]
    );
    assert!(
        requests[0].contains("includes%5B%5D=cover_art")
            || requests[0].contains("includes[]=cover_art")
    );
    assert_discreet(&requests[0], "title=one");
    assert!(
        requests[0].contains("contentRating%5B%5D=suggestive")
            || requests[0].contains("contentRating[]=suggestive")
    );
    assert!(!requests[0].contains("erotica"), "{}", requests[0]);
    assert!(requests[1].starts_with(&format!("GET /manga/{id}?")));
    drop(requests);
    // Un identifiant qui n'est pas un UUID ne part pas.
    assert_eq!(mangadex.fetch("../../admin"), Err(FetchError::NotFound));
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}

#[test]
fn ids_name_their_source() {
    assert_eq!(Anilist::new("http://x").id(), SourceId::Anilist);
    assert_eq!(
        serde_json::to_string(&SourceId::Mangadex).unwrap(),
        r#""mangadex""#
    );
}

#[test]
fn redirects_are_never_followed() {
    // Une réponse 302 vers le réseau local : rien n'est suivi, une seule requête part.
    let server = mock(vec![(
        302,
        "location: http://127.0.0.1:1/admin\r\n",
        String::new(),
    )]);
    assert_eq!(
        Mangadex::new(&server.url).search("x", false),
        Err(FetchError::Http(302))
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}

#[test]
fn adult_entries_only_when_allowed() {
    let server = mock(vec![(200, "", r#"{"data":{"Page":{"media":[]}}}"#.into())]);
    Anilist::new(&server.url).search("x", true).unwrap();
    assert!(!server.requests.lock().unwrap()[0].contains("isAdult"));
    let server = mock(vec![(200, "", r#"{"data":[]}"#.into())]);
    Mangadex::new(&server.url).search("x", true).unwrap();
    let request = server.requests.lock().unwrap()[0].clone();
    assert!(
        request.contains("pornographic") && request.contains("erotica"),
        "{request}"
    );
}
