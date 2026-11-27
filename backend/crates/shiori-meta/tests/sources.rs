//! Clients AniList, MangaDex, Wikipédia et MangaUpdates contre un faux serveur local qui rejoue des réponses de la forme
//! des vraies (relevées le 2026-11-13, textes remplacés) : aucun appel réseau dans les tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use shiori_meta::{
    Anilist, Author, Comicvine, FetchError, Mangadex, Mangaupdates, Source, SourceId, Text,
    WebMetadata, Wikipedia,
};

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
        .search("one piece", false, "fr")
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
        mangadex.search("akira", false, "fr"),
        Err(FetchError::RateLimited(Duration::from_secs(30)))
    );
    // Pendant l'attente demandée, rien ne part : la requête suivante renonce aussitôt.
    assert!(matches!(
        mangadex.search("akira", false, "fr"),
        Err(FetchError::RateLimited(_))
    ));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    // Sans `Retry-After`, une minute.
    let server = mock(vec![(429, "", String::new())]);
    assert_eq!(
        Anilist::new(&server.url).search("x", false, "fr"),
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
        Mangadex::new(&url).search("x", false, "fr"),
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
    let found = mangadex.search("one piece", false, "fr").unwrap();
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
        Mangadex::new(&server.url).search("x", false, "fr"),
        Err(FetchError::Http(302))
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}

#[test]
fn adult_entries_only_when_allowed() {
    let server = mock(vec![(200, "", r#"{"data":{"Page":{"media":[]}}}"#.into())]);
    Anilist::new(&server.url).search("x", true, "fr").unwrap();
    assert!(!server.requests.lock().unwrap()[0].contains("isAdult"));
    let server = mock(vec![(200, "", r#"{"data":[]}"#.into())]);
    Mangadex::new(&server.url).search("x", true, "fr").unwrap();
    let request = server.requests.lock().unwrap()[0].clone();
    assert!(
        request.contains("pornographic") && request.contains("erotica"),
        "{request}"
    );
}

#[test]
fn wikipedia_search_keeps_comics_articles_of_the_interface_language() {
    let answer = r#"{"batchcomplete":true,"query":{"pages":[
        {"pageid":2,"title":"Dofus (manga)","index":2,"extract":"Dofus est une série de bandes dessinées.\n","description":"série de bandes dessinées","fullurl":"https://fr.wikipedia.org/wiki/Dofus_(manga)"},
        {"pageid":1,"title":"Dofus","index":1,"extract":"Dofus est un jeu vidéo.","description":"jeu vidéo de 2004","fullurl":"https://fr.wikipedia.org/wiki/Dofus"},
        {"pageid":3,"title":"Dofus (homonymie)","index":3,"pageprops":{"disambiguation":""},"fullurl":"https://fr.wikipedia.org/wiki/Dofus_(homonymie)"},
        {"pageid":4,"title":"Wakfu","index":4,"extract":"","fullurl":"https://fr.wikipedia.org/wiki/Wakfu"}
    ]}}"#;
    let server = mock(vec![(200, "", answer.into())]);
    let found = Wikipedia::new(&server.url)
        .search("Dofus", false, "fr")
        .unwrap();
    assert_eq!(
        found,
        [
            WebMetadata {
                id: "fr:Dofus (manga)".into(),
                titles: vec![text(Some("fr"), "Dofus")],
                summaries: vec![text(Some("fr"), "Dofus est une série de bandes dessinées.")],
                links: vec!["https://fr.wikipedia.org/wiki/Dofus_(manga)".into()],
                ..WebMetadata::default()
            },
            // Sans description, l'article reste ; une introduction vide ne fait pas de résumé.
            WebMetadata {
                id: "fr:Wakfu".into(),
                titles: vec![text(Some("fr"), "Wakfu")],
                links: vec!["https://fr.wikipedia.org/wiki/Wakfu".into()],
                ..WebMetadata::default()
            },
        ]
    );
    let request = &server.requests.lock().unwrap()[0];
    assert!(request.starts_with("GET /w/api.php?"), "{request}");
    assert!(request.contains("maxlag=5"), "{request}");
    // Wikimedia exige un contact : celui du projet, rien sur l'utilisateur.
    assert!(
        request
            .to_ascii_lowercase()
            .contains("(https://github.com/befa160/shiori)"),
        "{request}"
    );
    assert_discreet(request, "gsrsearch=Dofus");
}

#[test]
fn wikipedia_fetch_keeps_both_languages_and_no_title() {
    let fr = r#"{"query":{"pages":[{"title":"Dofus (manga)","extract":"Résumé français.","fullurl":"https://fr.wikipedia.org/wiki/Dofus_(manga)","langlinks":[{"lang":"en","title":"Dofus (comics)"}]}]}}"#;
    let en = r#"{"query":{"pages":[{"title":"Dofus (comics)","extract":"English summary.","fullurl":"https://en.wikipedia.org/wiki/Dofus_(comics)"}]}}"#;
    let server = mock(vec![(200, "", fr.into()), (200, "", en.into())]);
    let data = Wikipedia::new(&server.url)
        .fetch("fr:Dofus (manga)")
        .unwrap();
    assert_eq!(
        data,
        WebMetadata {
            id: "fr:Dofus (manga)".into(),
            summaries: vec![
                text(Some("fr"), "Résumé français."),
                text(Some("en"), "English summary.")
            ],
            links: vec![
                "https://fr.wikipedia.org/wiki/Dofus_(manga)".into(),
                "https://en.wikipedia.org/wiki/Dofus_(comics)".into()
            ],
            ..WebMetadata::default()
        }
    );
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].contains("lllang=en"), "{}", requests[0]);
    assert!(
        requests[1].contains("titles=Dofus%20(comics)"),
        "{}",
        requests[1]
    );
}

#[test]
fn wikipedia_reports_unknown_articles_and_waits_on_maxlag() {
    // Identifiant d'une autre langue (ou d'un autre hôte) : rien ne part.
    let wiki = Wikipedia::new("http://127.0.0.1:1");
    for id in ["de:Dofus", "evil.example/x:Dofus", "fr:", "Dofus"] {
        assert_eq!(wiki.fetch(id), Err(FetchError::NotFound), "{id}");
    }
    assert_eq!(wiki.image("https://x"), Err(FetchError::NotFound));
    let missing = r#"{"query":{"pages":[{"title":"Inconnu","missing":true}]}}"#;
    let lagging = r#"{"error":{"code":"maxlag","info":"Waiting for a database server"}}"#;
    let other = r#"{"error":{"code":"badvalue","info":"x"}}"#;
    let server = mock(vec![
        (200, "", missing.into()),
        (200, "", lagging.into()),
        (200, "", other.into()),
        (200, "", "{}".into()),
    ]);
    let wiki = Wikipedia::new(&server.url);
    assert_eq!(wiki.fetch("en:Inconnu"), Err(FetchError::NotFound));
    assert!(matches!(
        wiki.search("x", false, "fr"),
        Err(FetchError::RateLimited(_))
    ));
    assert_eq!(
        wiki.search("x", false, "fr"),
        Err(FetchError::BadResponse("badvalue".into()))
    );
    // Rien trouvé ; une langue inconnue cherche en anglais.
    assert_eq!(wiki.search("x", false, "de"), Ok(vec![]));
    assert_eq!(wiki.id(), SourceId::Wikipedia);
}

#[test]
fn a_wikipedia_article_in_one_language_only_still_links() {
    // Pas d'article anglais (cas courant pour la BD franco-belge) : une seule requête.
    let alone = r#"{"query":{"pages":[{"title":"Dofus (manfra)","extract":"Résumé.","fullurl":"https://fr.wikipedia.org/wiki/Dofus_(manfra)"}]}}"#;
    let server = mock(vec![(200, "", alone.into())]);
    let data = Wikipedia::new(&server.url)
        .fetch("fr:Dofus (manfra)")
        .unwrap();
    assert_eq!(data.summaries, [text(Some("fr"), "Résumé.")]);
    assert_eq!(data.links, ["https://fr.wikipedia.org/wiki/Dofus_(manfra)"]);
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    // Un lien interlangue vers une page disparue : la fiche garde sa seule langue.
    let linked = r#"{"query":{"pages":[{"title":"Akira","extract":"Résumé.","langlinks":[{"lang":"en","title":"Akira (gone)"}]}]}}"#;
    let gone = r#"{"query":{"pages":[{"title":"Akira (gone)","missing":true}]}}"#;
    let server = mock(vec![(200, "", linked.into()), (200, "", gone.into())]);
    let data = Wikipedia::new(&server.url).fetch("fr:Akira").unwrap();
    assert_eq!((data.summaries.len(), data.links.len()), (1, 0));
}

#[test]
fn mangaupdates_search_and_fetch_are_normalized() {
    let server = mock(vec![
        (200, "", fixture("mangaupdates_search.json")),
        (200, "", fixture("mangaupdates_series.json")),
        (404, "", r#"{"status":"exception","reason":"x"}"#.into()),
    ]);
    let source = Mangaupdates::new(&server.url);
    let found = source.search("cagaster", false, "fr").unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0],
        WebMetadata {
            id: "39150200246".into(),
            // Le titre qui a répondu à la recherche compte pour le rapprochement.
            titles: vec![text(None, "Mushikago no Cagaster"), text(None, "Cagaster")],
            summaries: vec![text(
                Some("en"),
                "Une peste & des insectes.\nDeux survivants.\n\n(Source: exemple)"
            )],
            authors: vec![],
            genres: vec!["Action".into(), "Seinen".into()],
            tags: vec![],
            year: Some(2015),
            status: None,
            links: vec!["https://www.mangaupdates.com/series/hzh0nue/mushikago-no-cagaster".into()],
            cover_url: Some("https://cdn.mangaupdates.com/image/i407606.jpg".into()),
        }
    );
    // Résumé vide, année vide, pas d'image.
    assert_eq!(
        (
            found[1].summaries.len(),
            found[1].year,
            found[1].cover_url.as_deref()
        ),
        (0, None, None)
    );
    let request = server.requests.lock().unwrap()[0].clone();
    assert!(request.starts_with("POST /series/search "), "{request}");
    assert!(
        request.contains(r#""exclude_genre":["Adult","Hentai","Smut"]"#),
        "{request}"
    );
    assert_discreet(&request, r#""search":"cagaster""#);

    let series = source.fetch("39150200246").unwrap();
    assert_eq!(series.titles.len(), 4);
    assert_eq!(series.status.as_deref(), Some("completed"));
    assert_eq!(
        series.authors,
        [
            author("HASHIMOTO Kachou", true, true),
            author("Dessinateur B", false, true)
        ]
    );
    assert!(server.requests.lock().unwrap()[1].starts_with("GET /series/39150200246 "));
    assert_eq!(source.fetch("1"), Err(FetchError::NotFound));
    // Rien d'autre qu'un nombre ne part dans le chemin.
    assert_eq!(source.fetch("../admin"), Err(FetchError::NotFound));
    assert_eq!(source.fetch(""), Err(FetchError::NotFound));
    assert_eq!(server.requests.lock().unwrap().len(), 3);
    assert_eq!(source.id(), SourceId::Mangaupdates);
}

#[test]
fn mangaupdates_adult_entries_only_when_allowed_and_covers_from_its_cdn() {
    let server = mock(vec![(200, "", r#"{"results":[]}"#.into())]);
    let source = Mangaupdates::new(&server.url);
    source.search("x", true, "fr").unwrap();
    assert!(!server.requests.lock().unwrap()[0].contains("exclude_genre"));
    assert!(matches!(
        source.image("https://evil.example/i.jpg"),
        Err(FetchError::BadResponse(_))
    ));
}

#[test]
fn comicvine_needs_a_key_sends_it_and_never_shows_it() {
    let server = mock(vec![
        (200, "", fixture("comicvine_search.json")),
        (
            200,
            "",
            r#"{"error":"OK","status_code":1,"results":{"id":7,"name":"Blacksad","aliases":"","start_year":"2000"}}"#.into(),
        ),
        (401, "", r#"{"error":"Invalid API Key","status_code":100}"#.into()),
        (200, "", r#"{"error":"Object Not Found","status_code":101,"results":[]}"#.into()),
        (420, "", String::new()),
    ]);
    let source = Comicvine::new(&server.url);
    assert_eq!(source.id(), SourceId::Comicvine);
    // Sans clé, rien ne part.
    assert_eq!(source.search("x", false, "fr"), Err(FetchError::MissingKey));
    assert!(server.requests.lock().unwrap().is_empty());

    source.set_key(Some("CLE0123456789".into()));
    let found = source.search("geezers", false, "fr").unwrap();
    assert_eq!(
        found[0],
        WebMetadata {
            id: "123456".into(),
            titles: vec![
                text(Some("en"), "Old Geezers"),
                text(None, "Les Vieux Fourneaux"),
                text(None, "The Old Geezers")
            ],
            summaries: vec![text(Some("en"), "Trois amis & septuagénaires.")],
            year: Some(2014),
            links: vec!["https://comicvine.gamespot.com/old-geezers/4050-123456/".into()],
            cover_url: Some("https://comicvine.gamespot.com/a/uploads/original/1/1/1-x.jpg".into()),
            ..WebMetadata::default()
        }
    );
    // Sans résumé court, l'article ; année en nombre.
    assert_eq!(
        (found[1].summaries[0].text.as_str(), found[1].year),
        ("Une BD.\n\nTome 1", Some(2000))
    );
    let request = server.requests.lock().unwrap()[0].clone();
    assert!(request.starts_with("GET /search/?"), "{request}");
    assert!(request.contains("api_key=CLE0123456789") && request.contains("resources=volume"));
    assert_discreet(&request, "query=geezers");

    assert_eq!(
        source.fetch("7").unwrap().titles,
        [text(Some("en"), "Blacksad")]
    );
    assert!(server.requests.lock().unwrap()[1].starts_with("GET /volume/4050-7/?"));
    assert_eq!(source.fetch("8"), Err(FetchError::InvalidKey));
    assert_eq!(source.search("x", false, "fr"), Err(FetchError::NotFound));
    assert!(matches!(
        source.search("x", false, "fr"),
        Err(FetchError::RateLimited(_))
    ));
    assert_eq!(source.fetch("../x"), Err(FetchError::NotFound));
    assert!(matches!(
        source.image("https://evil.example/x.jpg"),
        Err(FetchError::BadResponse(_))
    ));
}

#[test]
fn a_comicvine_network_error_hides_the_key() {
    let source = Comicvine::new("http://127.0.0.1:1");
    source.set_key(Some("SECRET42".into()));
    let error = source.search("x", false, "fr").unwrap_err();
    assert!(matches!(error, FetchError::Offline(_)), "{error:?}");
    assert!(
        !format!("{error:?} {error}").contains("SECRET42"),
        "{error:?}"
    );
}
