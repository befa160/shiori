use super::*;

fn limit(key: &str, free: u32) -> [Limit; 1] {
    [Limit {
        key: key.to_owned(),
        free,
        max: MAX_PAUSE,
        reset: true,
    }]
}

fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(*name, HeaderValue::from_str(value).unwrap());
    }
    map
}

#[test]
fn the_client_address_trusts_a_forwarded_header_only_from_a_local_proxy() {
    let forwarded = headers(&[("x-forwarded-for", "6.6.6.6, 203.0.113.7")]);
    let local: SocketAddr = "127.0.0.1:5000".parse().unwrap();
    let lan: SocketAddr = "192.168.1.2:5000".parse().unwrap();
    let ula: SocketAddr = "[fd00::1]:5000".parse().unwrap();
    let public: SocketAddr = "198.51.100.1:5000".parse().unwrap();
    // Le proxy ajoute l'adresse qu'il voit à la fin : la première a pu être inventée.
    assert_eq!(client_address(Some(local), &forwarded, &[]), "203.0.113.7");
    assert_eq!(client_address(Some(lan), &forwarded, &[]), "203.0.113.7");
    assert_eq!(client_address(Some(ula), &forwarded, &[]), "203.0.113.7");
    assert_eq!(
        client_address(Some(public), &forwarded, &[]),
        "198.51.100.1"
    );
    assert_eq!(
        client_address(Some(local), &HeaderMap::new(), &[]),
        "127.0.0.1"
    );
    let junk = headers(&[("x-forwarded-for", "pas une adresse")]);
    assert_eq!(client_address(Some(local), &junk, &[]), "127.0.0.1");
    assert_eq!(client_address(None, &forwarded, &[]), "inconnue");
    // Proxies donnés (`--trusted-proxy`) : seuls eux sont crus, même sur le réseau local.
    let proxy = ["192.168.1.2".parse().unwrap()];
    let other: SocketAddr = "192.168.1.3:5000".parse().unwrap();
    assert_eq!(client_address(Some(lan), &forwarded, &proxy), "203.0.113.7");
    assert_eq!(
        client_address(Some(other), &forwarded, &proxy),
        "192.168.1.3"
    );
    assert_eq!(client_address(Some(local), &forwarded, &proxy), "127.0.0.1");
    let mapped: SocketAddr = "[::ffff:192.168.1.2]:5000".parse().unwrap();
    assert_eq!(
        client_address(Some(mapped), &forwarded, &proxy),
        "203.0.113.7"
    );
}

#[test]
fn the_token_comes_from_bearer_first_then_from_the_cookie() {
    let both = headers(&[
        ("authorization", "Bearer shiori_s_app"),
        ("cookie", "shiori_session=shiori_s_web"),
    ]);
    assert_eq!(presented_token(&both), Some(("shiori_s_app".into(), false)));
    let cookies = headers(&[
        ("cookie", "theme=dark"),
        ("cookie", "a=1; shiori_session=shiori_s_web; b=2"),
    ]);
    assert_eq!(
        presented_token(&cookies),
        Some(("shiori_s_web".into(), true))
    );
    let other = headers(&[
        ("authorization", "Basic xyz"),
        ("cookie", "shiori_session_old=x"),
    ]);
    assert_eq!(presented_token(&other), None);
}

#[test]
fn the_cookie_is_secure_behind_an_https_proxy() {
    let plain = cookie(&HeaderMap::new(), "t", 60);
    assert_eq!(
        plain,
        "shiori_session=t; Path=/; HttpOnly; SameSite=Lax; Max-Age=60"
    );
    let https = cookie(&headers(&[("x-forwarded-proto", "HTTPS")]), "t", 0);
    assert!(https.to_str().unwrap().ends_with("; Max-Age=0; Secure"));
}

#[test]
fn failures_pause_after_the_free_attempts_then_double() {
    let guard = Guard::new();
    let keys = limit("nom:bob", 2);
    for _ in 0..2 {
        assert_eq!(guard.paused(&keys), None);
        guard.failed(&keys);
    }
    assert_eq!(guard.paused(&keys), None, "deux essais ratés sont gratuits");
    guard.failed(&keys);
    let first = guard.paused(&keys).unwrap();
    assert!(
        first <= Duration::from_secs(2) && first > Duration::from_secs(1),
        "{first:?}"
    );
    guard.failed(&keys);
    assert!(guard.paused(&keys).unwrap() > Duration::from_secs(3));
    for _ in 0..30 {
        guard.failed(&keys);
    }
    assert!(guard.paused(&keys).unwrap() <= MAX_PAUSE);
    // Une autre clé n'est pas touchée ; une connexion réussie efface le compteur du nom.
    assert_eq!(guard.paused(&limit("nom:alice", 2)), None);
    guard.succeeded(&keys);
    assert_eq!(guard.paused(&keys), None);
}

#[test]
fn old_failures_are_forgotten_when_the_table_grows() {
    let guard = Guard::new();
    {
        let mut failures = guard.failures.lock().unwrap();
        let old = Instant::now().checked_sub(MAX_PAUSE * 2).unwrap();
        for i in 0..10_001 {
            failures.insert(format!("nom:{i}"), (1, old));
        }
    }
    guard.failed(&limit("nom:récent", 5));
    assert_eq!(guard.failures.lock().unwrap().len(), 1);
}

#[test]
fn secrets_are_compared_whole() {
    assert!(same_secret("abc", "abc"));
    assert!(!same_secret("abc", "abd"));
    assert!(!same_secret("abc", "abcd"));
}

#[test]
fn expired_challenges_are_dropped_when_a_new_one_comes() {
    let guard = Guard::new();
    let old = Instant::now().checked_sub(CHALLENGE + Duration::from_secs(1));
    let Some(old) = old else { return }; // horloge trop jeune (démarrage de la machine) : rien à tester
    let challenge = |at| Challenge {
        user_id: 1,
        client: Client::Browser,
        limits: login_limits("a", "b"),
        at,
    };
    let stale = guard.challenge(challenge(old));
    let fresh = guard.challenge(challenge(Instant::now()));
    let challenges = guard.challenges.lock().unwrap();
    // Le défi périmé est parti à l'arrivée du second : la table ne grossit pas.
    assert!(challenges.contains_key(&fresh));
    assert!(!challenges.contains_key(&stale));
    assert_eq!(challenges.len(), 1);
}

#[test]
fn an_api_token_reads_and_saves_progress_nothing_else() {
    let allowed = [
        (Method::GET, "/api/series"),
        (Method::GET, "/api/pages/abc/image"),
        (Method::HEAD, "/api/covers/x"),
        (Method::PUT, "/api/volumes/abc/progress"),
    ];
    for (method, path) in allowed {
        assert!(token_allows(&method, path), "{method} {path}");
    }
    let refused = [
        (Method::GET, "/api/me"),
        (Method::GET, "/api/me/tokens"),
        (Method::GET, "/api/admin/users"),
        (Method::DELETE, "/api/volumes/abc/progress"),
        (Method::PUT, "/api/volumes//progress"),
        (Method::PUT, "/api/volumes/a/b/progress"),
        (Method::PUT, "/api/settings"),
        (Method::PATCH, "/api/series/1"),
        (Method::POST, "/api/libraries"),
    ];
    for (method, path) in refused {
        assert!(!token_allows(&method, path), "{method} {path}");
    }
}

#[test]
fn wrong_passwords_pause_the_name_from_that_address_only() {
    let guard = Guard::new();
    let here = login_limits("Bob", "203.0.113.7");
    for _ in 0..6 {
        guard.failed(&here);
    }
    assert!(guard.paused(&here).is_some());
    // Bob, depuis chez lui : pas bloqué par celui qui se trompe sur son nom ailleurs.
    assert_eq!(guard.paused(&login_limits("bob", "198.51.100.1")), None);
    // Essais répartis sur beaucoup d'adresses : le nom finit par attendre, une minute au plus.
    for n in 0..50 {
        guard.failed(&login_limits("bob", &format!("10.0.0.{n}")));
    }
    let wait = guard.paused(&login_limits("bob", "198.51.100.1")).unwrap();
    assert!(wait <= MAX_PAUSE_ANYWHERE, "{wait:?}");
    // Une connexion réussie remet le nom à zéro, pas l'adresse (partagée, derrière une box).
    guard.succeeded(&here);
    assert_eq!(guard.paused(&login_limits("bob", "198.51.100.1")), None);
    for _ in 0..20 {
        guard.failed(&login_limits("alice", "203.0.113.7"));
    }
    guard.succeeded(&login_limits("alice", "203.0.113.7"));
    assert!(
        guard
            .paused(&login_limits("carol", "203.0.113.7"))
            .is_some()
    );
}
