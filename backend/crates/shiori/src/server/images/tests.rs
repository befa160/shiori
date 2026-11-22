use super::*;

/// L'anticipation ne prend jamais le dernier jeton de rendu : elle renonce sans lire l'originale,
/// et une vraie demande passe quand même.
#[tokio::test]
async fn rendering_ahead_never_takes_the_last_render_slot() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(shiori_db::Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let app = Arc::new(App::new(db, images));
    let id: PageId = "00".repeat(16).parse().unwrap();
    let free = app.renders.available_permits();
    let _held = app.renders.acquire_many(free as u32 - 1).await.unwrap();
    // Lue, l'originale donnerait une erreur : `Ok(None)` prouve qu'elle ne l'a pas été.
    let never = async { Err(corrupt("l'originale ne doit pas être lue")) };
    let ahead = render_cached(&app, id, 320, Format::Jpeg, never, false).await;
    assert!(matches!(ahead, Ok(Render::NoSlot)));
    let page = shiori_testkit::png(640, 960, [1, 2, 3]);
    let now = render_cached(&app, id, 320, Format::Jpeg, async { Ok(page) }, true).await;
    assert!(matches!(now, Ok(Render::Done(_))));
}

/// Une page déjà en cours de rendu n'est jamais rendue une deuxième fois : l'anticipation la passe,
/// une vraie demande attend la fin et sert le résultat mis en cache (vu sur le NAS : 7 pages sur 60
/// rendues deux fois, à quelques millisecondes d'écart).
#[tokio::test]
async fn a_page_in_flight_is_never_rendered_twice() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(shiori_db::Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let app = Arc::new(App::new(db, images));
    let id: PageId = "11".repeat(16).parse().unwrap();
    let key = (id, 320, Format::Jpeg);
    app.rendering.lock().unwrap().insert(key, Arc::default());
    let first = InFlight(&app, key);
    // Lue, l'originale donnerait une erreur : elle ne doit l'être par aucun des deux.
    let unread = || async { Err(corrupt("l'originale ne doit pas être relue")) };
    let ahead = render_cached(&app, id, 320, Format::Jpeg, unread(), false).await;
    assert!(matches!(ahead, Ok(Render::Elsewhere)));
    let waiting = tokio::spawn({
        let app = app.clone();
        async move { render_cached(&app, id, 320, Format::Jpeg, unread(), true).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    assert!(
        !waiting.is_finished(),
        "la vraie demande attend le rendu en cours"
    );
    // Le premier rendu se termine : son résultat est en cache, la demande est réveillée et le sert
    // (sans attendre une vérification périodique : une seconde suffit largement).
    app.images.put(&id, 320, Format::Jpeg, b"rendu").unwrap();
    drop(first);
    let served = tokio::time::timeout(std::time::Duration::from_secs(1), waiting).await;
    match served.expect("la demande est réveillée").unwrap() {
        Ok(Render::Done(bytes)) => assert_eq!(bytes, b"rendu"),
        _ => panic!("la demande sert le rendu déjà fait"),
    }
}
