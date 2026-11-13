use super::*;

fn titled(titles: &[&str]) -> WebMetadata {
    WebMetadata {
        titles: titles
            .iter()
            .map(|t| Text {
                lang: None,
                text: (*t).into(),
            })
            .collect(),
        ..WebMetadata::default()
    }
}

#[test]
fn titles_are_compared_without_case_accents_or_punctuation() {
    assert_eq!(normalize("One-Piece !"), "onepiece");
    assert_eq!(normalize("Été en Pente"), normalize("ete en pente"));
    assert_eq!(normalize("ワンピース"), "ワンピース");
    assert_eq!(normalize(" - "), "");
}

#[test]
fn only_a_single_exact_candidate_is_matched() {
    let candidates = [
        titled(&["Wan Piece"]),
        titled(&["ONE PIECE", "ワンピース"]),
        titled(&["One Piece Party"]),
    ];
    assert_eq!(auto_match(&["One Piece"], &candidates), Some(1));
    // Le nom du dossier ou le titre des fichiers peuvent correspondre.
    assert_eq!(auto_match(&["Dossier", "ワンピース"], &candidates), Some(1));
    // Deux candidats exacts : ambigu, à confirmer.
    let twice = [titled(&["Akira"]), titled(&["AKIRA"])];
    assert_eq!(auto_match(&["Akira"], &twice), None);
    assert_eq!(auto_match(&["Inconnu"], &candidates), None);
    // Vu chez AniList : la parodie « Wan Piece » a « One Piece » en synonyme (langue inconnue) ;
    // le titre principal départage.
    let main = |text: &str| Text {
        lang: Some("en".into()),
        text: text.into(),
    };
    let mut real = titled(&[]);
    real.titles.push(main("One Piece"));
    let parody = titled(&["Wan Piece", "One Piece"]);
    assert_eq!(
        auto_match(&["One Piece"], &[real.clone(), parody.clone()]),
        Some(0)
    );
    // Seul un synonyme correspond : il suffit s'il est unique.
    assert_eq!(auto_match(&["Wan Piece"], &[real, parody]), Some(1));
    // Un nom vide ne correspond à rien, même à un titre vide.
    assert_eq!(auto_match(&["!!"], &[titled(&["?"])]), None);
}

#[test]
fn descriptions_lose_their_markup() {
    let text = "Luffy &amp; ses amis.\n<br><br>\n<i>Le &quot;One Piece&quot;</i> l&#039;attend.<br/>(Source: X)";
    assert_eq!(
        plain(text),
        "Luffy & ses amis.\n\nLe \"One Piece\" l'attend.\n(Source: X)"
    );
    assert_eq!(plain("a < b et pas de balise"), "a < b et pas de balise");
    assert_eq!(plain("  <BR>  "), "");
}

#[test]
fn authors_gather_their_roles() {
    let mut authors = Vec::new();
    add_author(&mut authors, "Oda", true, false);
    add_author(&mut authors, " Oda ", false, true);
    add_author(&mut authors, "  ", true, true);
    assert_eq!(
        authors,
        [Author {
            name: "Oda".into(),
            story: true,
            art: true
        }]
    );
}

#[test]
fn errors_have_stable_codes() {
    let codes = [
        FetchError::Offline("x".into()),
        FetchError::RateLimited(Duration::ZERO),
        FetchError::NotFound,
        FetchError::Http(500),
        FetchError::BadResponse("x".into()),
    ]
    .map(|e| e.code());
    assert_eq!(
        codes,
        [
            "offline",
            "rate_limited",
            "not_found",
            "http_error",
            "bad_response"
        ]
    );
}

#[test]
fn requests_keep_their_distance_and_give_up_on_a_long_wait() {
    let http = Http::new(Duration::from_millis(50));
    let start = Instant::now();
    http.turn().unwrap();
    http.turn().unwrap();
    http.turn().unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(100),
        "{:?}",
        start.elapsed()
    );
    // Une source a demandé d'attendre une minute : on renonce aussitôt.
    *http.next.lock().unwrap() = Instant::now() + Duration::from_secs(60);
    let start = Instant::now();
    assert!(matches!(http.turn(), Err(FetchError::RateLimited(_))));
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn only_https_addresses_on_the_expected_hosts_are_downloaded() {
    assert_eq!(
        image_host("https://S4.AniList.co/file/x.jpg").as_deref(),
        Some("s4.anilist.co")
    );
    assert_eq!(
        image_host("https://uploads.mangadex.org?x").as_deref(),
        Some("uploads.mangadex.org")
    );
    for refused in [
        "http://s4.anilist.co/x.jpg",
        "https://user@s4.anilist.co/x.jpg",
        "https://s4.anilist.co:8443/x.jpg",
        "https://127.0.0.1:80/",
        "https:///x",
        "file:///etc/passwd",
        "https://s4.anilist.co\\@evil/",
    ] {
        assert_eq!(image_host(refused), None, "{refused}");
    }
    // Un hôte inattendu n'est même pas contacté.
    let http = Http::new(Duration::ZERO);
    let evil = http.image("https://evil.example/x.jpg", |h| h.ends_with(".anilist.co"));
    assert!(matches!(evil, Err(FetchError::BadResponse(_))));
    let lookalike = http.image("https://anilist.co.evil.example/x.jpg", |h| {
        h.ends_with(".anilist.co")
    });
    assert!(matches!(lookalike, Err(FetchError::BadResponse(_))));
}
