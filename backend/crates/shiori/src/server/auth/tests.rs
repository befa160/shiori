use super::*;

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
    assert_eq!(client_address(Some(local), &forwarded), "203.0.113.7");
    assert_eq!(client_address(Some(lan), &forwarded), "203.0.113.7");
    assert_eq!(client_address(Some(ula), &forwarded), "203.0.113.7");
    assert_eq!(client_address(Some(public), &forwarded), "198.51.100.1");
    assert_eq!(client_address(Some(local), &HeaderMap::new()), "127.0.0.1");
    let junk = headers(&[("x-forwarded-for", "pas une adresse")]);
    assert_eq!(client_address(Some(local), &junk), "127.0.0.1");
    assert_eq!(client_address(None, &forwarded), "inconnue");
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
    let keys = [("nom:bob".to_owned(), 2)];
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
    assert_eq!(guard.paused(&[("nom:alice".to_owned(), 2)]), None);
    guard.succeeded("nom:bob");
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
    guard.failed(&[("nom:récent".to_owned(), 5)]);
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
        keys: [("nom:a".into(), 5), ("adresse:b".into(), 20)],
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
