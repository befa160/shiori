use serde_json::json;

use super::*;

fn author(name: &str) -> Author {
    Author {
        name: name.into(),
        story: true,
        art: false,
    }
}

fn comic(series: &str, summary: &str, year: &str) -> (MetadataSource, LocalMetadata) {
    (
        MetadataSource::Comicinfo,
        LocalMetadata {
            series: Some(series.into()),
            title: Some(format!("Titre {year}")),
            number: Some("1".into()),
            summary: Some(summary.into()),
            authors: vec![author("Oda")],
            genres: vec!["Aventure".into()],
            release_date: Some(format!("{year}-07")),
            right_to_left: Some(true),
            ..LocalMetadata::default()
        },
    )
}

fn edited(value: Value) -> Edited {
    value.as_object().unwrap().clone()
}

#[test]
fn index_json_describes_the_series_then_the_first_comic_info() {
    let index = (
        MetadataSource::IndexJson,
        LocalMetadata {
            series: Some("Leviathan".into()),
            alt_titles: vec!["Léviathan".into(), "leviathan".into()],
            summary: Some("Série".into()),
            status: Some("completed".into()),
            tags: vec!["SF".into()],
            ..LocalMetadata::default()
        },
    );
    let locals = [
        comic("One Piece", "Tome 1", "1997"),
        comic("Autre", "Tome 2", "1998"),
        index,
    ];
    let meta = from_locals("leviathan", &locals);
    assert_eq!(
        meta,
        SeriesMetadata {
            // Titre des fichiers (index.json d'abord) ; les autres restent alternatifs, sans
            // répéter le titre affiché ni le nom du dossier qui lui est égal, doublons sans casse retirés.
            title: Some("Leviathan".into()),
            alt_titles: vec!["Léviathan".into(), "One Piece".into(), "Autre".into()],
            summary: Some("Série".into()),
            authors: vec![author("Oda")],
            genres: vec!["Aventure".into()],
            tags: vec!["SF".into()],
            year: Some(1997),
            publication_status: Some(PublicationStatus::Completed),
            right_to_left: Some(true),
            ..SeriesMetadata::default()
        }
    );
    assert_eq!(from_locals("x", &[]), SeriesMetadata::default());
    // Un dossier nommé autrement reste cherchable.
    let meta = from_locals("Webtoon", &locals[2..]);
    assert_eq!(meta.alt_titles, ["Léviathan", "Webtoon"]);
}

#[test]
fn edited_fields_win_and_unlocking_gives_them_back() {
    let base = from_locals("x", &[comic("One Piece", "Tome 1", "1997")]);
    let mut fields = Edited::new();
    let patch = edited(json!({"summary": "À moi", "title": "One Piece", "year": null}));
    apply(&mut fields, patch, SERIES_FIELDS).unwrap();
    let meta = overlay(base.clone(), &fields).unwrap();
    assert_eq!(meta.summary.as_deref(), Some("À moi"));
    assert_eq!(meta.title.as_deref(), Some("One Piece"));
    assert_eq!(meta.year, None, "null édité : verrouillé vide");
    assert_eq!(meta.genres, base.genres);

    apply(
        &mut fields,
        edited(json!({"unlock": ["summary", "year"]})),
        SERIES_FIELDS,
    )
    .unwrap();
    let meta = overlay(base, &fields).unwrap();
    assert_eq!(meta.summary.as_deref(), Some("Tome 1"));
    assert_eq!(meta.year, Some(1997));
    assert_eq!(fields.keys().collect::<Vec<_>>(), ["title"]);
}

#[test]
fn typed_text_is_trimmed_and_empty_text_is_null() {
    let mut fields = Edited::new();
    let patch =
        edited(json!({"title": "   ", "summary": " Résumé ", "genres": [" SF ", "", "  "]}));
    apply(&mut fields, patch, SERIES_FIELDS).unwrap();
    let meta = overlay(SeriesMetadata::default(), &fields).unwrap();
    assert_eq!(
        meta.title, None,
        "un titre effacé retombe sur le nom du dossier"
    );
    assert_eq!(meta.summary.as_deref(), Some("Résumé"));
    assert_eq!(meta.genres, ["SF"]);
    assert!(fields.contains_key("title"), "toujours verrouillé");
}

#[test]
fn bad_patches_are_refused() {
    let mut fields = Edited::new();
    for patch in [
        json!({"nope": 1}),
        json!({"unlock": "title"}),
        json!({"unlock": ["right_to_left"]}),
        json!({"right_to_left": true}),
    ] {
        assert!(
            apply(&mut fields, edited(patch.clone()), SERIES_FIELDS).is_err(),
            "{patch}"
        );
    }
    apply(
        &mut fields,
        edited(json!({"year": "deux mille"})),
        SERIES_FIELDS,
    )
    .unwrap();
    assert!(overlay(SeriesMetadata::default(), &fields).is_err());
    let mut fields = Edited::new();
    assert!(apply(&mut fields, edited(json!({"genres": []})), VOLUME_FIELDS).is_err());
}

#[test]
fn a_pdf_describes_its_volume_and_comes_after_comic_info() {
    let pdf = (
        MetadataSource::Pdf,
        LocalMetadata {
            summary: Some("Du PDF".into()),
            authors: vec![author("Kaku")],
            tags: vec!["Ninja".into()],
            ..LocalMetadata::default()
        },
    );
    assert_eq!(
        volume_from_locals(std::slice::from_ref(&pdf)),
        VolumeMetadata {
            summary: Some("Du PDF".into()),
            authors: vec![author("Kaku")],
            ..VolumeMetadata::default()
        }
    );
    // Dans une série, un ComicInfo.xml l'emporte sur un PDF, même s'il vient après.
    let meta = from_locals("x", &[pdf, comic("One Piece", "Tome 1", "1997")]);
    assert_eq!(meta.summary.as_deref(), Some("Tome 1"));
    assert_eq!(meta.tags, ["Ninja"]);
}

#[test]
fn a_volume_reads_only_its_comic_info() {
    let index = (
        MetadataSource::IndexJson,
        LocalMetadata {
            summary: Some("Série".into()),
            ..LocalMetadata::default()
        },
    );
    assert_eq!(
        volume_from_locals(std::slice::from_ref(&index)),
        VolumeMetadata::default()
    );
    assert_eq!(
        volume_from_locals(&[index, comic("One Piece", "Tome 1", "1997")]),
        VolumeMetadata {
            title: Some("Titre 1997".into()),
            number: Some("1".into()),
            summary: Some("Tome 1".into()),
            release_date: Some("1997-07".into()),
            authors: vec![author("Oda")],
            cover_page_id: None,
        }
    );
}

mod db {
    use serde_json::json;
    use shiori_core::ScanOptions;
    use shiori_testkit::{DEFLATED, page_color, png, write_zip};

    use crate::{Db, LibraryKind, PublicationStatus};

    fn patch(value: serde_json::Value) -> crate::Edited {
        value.as_object().unwrap().clone()
    }

    #[tokio::test]
    async fn scan_edit_rescan_unlock_and_cover() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).await.unwrap();
        let root = dir.path().join("manga");
        let folder = root.join("One Piece");
        std::fs::create_dir_all(&folder).unwrap();
        let library = db
            .create_library("M", &root, LibraryKind::Manga)
            .await
            .unwrap();
        let page = png(4, 6, page_color(0));
        let info = br#"<ComicInfo><Series>ONE PIECE</Series><Title>Romance Dawn</Title>
            <Writer>Eiichiro Oda</Writer><Summary>Luffy.</Summary></ComicInfo>"#;
        let file = write_zip(
            &folder,
            "One Piece T01.cbz",
            &[
                ("01.png", &page, DEFLATED),
                ("02.png", &page, DEFLATED),
                ("ComicInfo.xml", info, DEFLATED),
            ],
        );
        let pass = async || {
            let run = db.start_run(&library.root, "{}").await.unwrap();
            db.scan(run, &file, ScanOptions::default(), true)
                .await
                .unwrap();
            db.attach(&library, &file).await.unwrap();
            db.refresh_series(&library).await.unwrap();
        };
        pass().await;

        let [s] = &db
            .all_series(&crate::tests::viewer(&db).await)
            .await
            .unwrap()[..]
        else {
            panic!()
        };
        let id = s.id;
        // Le titre du fichier passe avant le nom du dossier ; égal à la casse près, celui-ci
        // n'est pas répété en titre alternatif.
        assert_eq!(s.name, "ONE PIECE");
        assert!(s.alt_titles.is_empty());
        assert_eq!(s.authors, ["Eiichiro Oda"]);
        let volume = db
            .series_detail(&crate::tests::viewer(&db).await, id)
            .await
            .unwrap()
            .unwrap()
            .volumes[0]
            .volume_id
            .clone();
        let volume: shiori_core::VolumeId = volume.parse().unwrap();
        let pages = db
            .pages_of(&crate::tests::viewer(&db).await, &volume)
            .await
            .unwrap()
            .unwrap();

        // Titre et couverture édités : la liste suit ; le rescan ne les touche pas.
        let edit =
            patch(json!({"title": "Wan Pīsu", "cover_page_id": pages[1].id, "summary": null}));
        assert!(db.edit_series(id, edit).await.unwrap());
        pass().await;
        let [s] = &db
            .all_series(&crate::tests::viewer(&db).await)
            .await
            .unwrap()[..]
        else {
            panic!()
        };
        assert_eq!(
            (s.name.as_str(), s.cover_page_id.as_deref()),
            ("Wan Pīsu", Some(pages[1].id.as_str()))
        );
        let detail = db
            .series_detail(&crate::tests::viewer(&db).await, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.name, "Wan Pīsu");
        assert_eq!(detail.metadata.summary, None);
        assert_eq!(detail.locked, ["cover_page_id", "summary", "title"]);

        // Rendu aux sources : la valeur du fichier revient.
        assert!(
            db.edit_series(id, patch(json!({"unlock": ["summary", "title"]})))
                .await
                .unwrap()
        );
        let detail = db
            .series_detail(&crate::tests::viewer(&db).await, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (detail.name.as_str(), detail.metadata.summary.as_deref()),
            ("ONE PIECE", Some("Luffy."))
        );

        // Refus : champ inconnu, mauvais type, page d'une autre série.
        for bad in [
            json!({"nope": 1}),
            json!({"year": "x"}),
            json!({"cover_page_id": "0".repeat(32)}),
        ] {
            assert!(matches!(
                db.edit_series(id, patch(bad)).await,
                Err(crate::EditError::Invalid(_))
            ));
        }

        // Tome : ComicInfo, puis titre édité.
        let v = db
            .volume_detail(&crate::tests::viewer(&db).await, &volume)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(v.metadata.title.as_deref(), Some("Romance Dawn"));
        // La liste des tomes de la série montre le même titre.
        let listed = &db
            .series_detail(&crate::tests::viewer(&db).await, id)
            .await
            .unwrap()
            .unwrap()
            .volumes[0];
        assert_eq!(listed.title.as_deref(), Some("Romance Dawn"));
        assert!(
            db.edit_volume(&volume, patch(json!({"title": "Tome 1"})))
                .await
                .unwrap()
        );
        let v = db
            .volume_detail(&crate::tests::viewer(&db).await, &volume)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (v.metadata.title.as_deref(), v.locked.as_slice()),
            (Some("Tome 1"), &["title".to_owned()][..])
        );
        let listed = &db
            .series_detail(&crate::tests::viewer(&db).await, id)
            .await
            .unwrap()
            .unwrap()
            .volumes[0];
        assert_eq!(listed.title.as_deref(), Some("Tome 1"));
        assert!(matches!(
            db.edit_volume(&volume, patch(json!({"genres": []}))).await,
            Err(crate::EditError::Invalid(_))
        ));
        let unknown: shiori_core::VolumeId = "0".repeat(64).parse().unwrap();
        assert!(!db.edit_volume(&unknown, patch(json!({}))).await.unwrap());

        // Couverture du tome : une de ses pages, montrée par la liste ; rendue, la page 0 revient.
        let cover_of = async || {
            db.series_detail(&crate::tests::viewer(&db).await, id)
                .await
                .unwrap()
                .unwrap()
                .volumes[0]
                .cover_page_id
                .clone()
        };
        assert_eq!(cover_of().await.as_deref(), Some(pages[0].id.as_str()));
        let edit = patch(json!({"cover_page_id": pages[1].id}));
        assert!(db.edit_volume(&volume, edit).await.unwrap());
        assert_eq!(cover_of().await.as_deref(), Some(pages[1].id.as_str()));
        assert!(matches!(
            db.edit_volume(&volume, patch(json!({"cover_page_id": "0".repeat(32)})))
                .await,
            Err(crate::EditError::Invalid(_))
        ));
        let edit = patch(json!({"unlock": ["cover_page_id"]}));
        assert!(db.edit_volume(&volume, edit).await.unwrap());
        assert_eq!(cover_of().await.as_deref(), Some(pages[0].id.as_str()));

        // Le fichier change : nouvelles pages, la couverture choisie disparaît.
        std::fs::remove_file(&file).unwrap();
        write_zip(
            &folder,
            "One Piece T01.cbz",
            &[("01.png", &png(4, 6, page_color(1)), DEFLATED)],
        );
        pass().await;
        let [s] = &db
            .all_series(&crate::tests::viewer(&db).await)
            .await
            .unwrap()[..]
        else {
            panic!()
        };
        assert_ne!(s.cover_page_id.as_deref(), Some(pages[1].id.as_str()));
        assert!(s.cover_page_id.is_some());
    }

    #[tokio::test]
    async fn simultaneous_edits_of_different_fields_are_both_kept() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).await.unwrap();
        let root = dir.path().join("m");
        std::fs::create_dir(&root).unwrap();
        let library = db
            .create_library("M", &root, LibraryKind::Manga)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO series (library_id, folder, name, created_at) VALUES (?, 'A', 'A', '')",
        )
        .bind(library.id)
        .execute(db.pool())
        .await
        .unwrap();
        let id = db
            .all_series(&crate::tests::viewer(&db).await)
            .await
            .unwrap()[0]
            .id;
        for round in 0..10 {
            let (a, b) = tokio::join!(
                db.edit_series(id, patch(json!({"summary": format!("r{round}")}))),
                db.edit_series(id, patch(json!({"genres": [format!("g{round}")]}))),
            );
            assert!(a.unwrap() && b.unwrap());
            let m = db
                .series_detail(&crate::tests::viewer(&db).await, id)
                .await
                .unwrap()
                .unwrap()
                .metadata;
            assert_eq!(
                (m.summary, m.genres),
                (Some(format!("r{round}")), vec![format!("g{round}")])
            );
        }
    }

    /// Le statut réglé à la main en phase 4 devient un champ édité, verrouillé.
    #[tokio::test]
    async fn phase_four_status_becomes_an_edited_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
        sqlx::migrate!().run_to(8, &pool).await.unwrap();
        sqlx::query(
            "INSERT INTO libraries (id, name, root, created_at) VALUES (1, 'L', '/l', '2026-01-02T00:00:00.000Z');
             INSERT INTO series (id, library_id, folder, name, publication_status) VALUES (1, 1, 'A', 'A', 'hiatus');
             INSERT INTO series (id, library_id, folder, name) VALUES (2, 1, 'B', 'B');",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let db = Db::open(&path).await.unwrap();
        let a = db
            .series_detail(&crate::tests::viewer(&db).await, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.publication_status, Some(PublicationStatus::Hiatus));
        assert_eq!(a.locked, ["publication_status"]);
        let b = db
            .series_detail(&crate::tests::viewer(&db).await, 2)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((b.publication_status, b.locked.len()), (None, 0));
    }
}

fn web(id: &str, titles: &[(Option<&str>, &str)], summaries: &[(&str, &str)]) -> WebMetadata {
    let text = |lang: Option<&str>, t: &str| Text {
        lang: lang.map(Into::into),
        text: t.into(),
    };
    WebMetadata {
        id: id.into(),
        titles: titles.iter().map(|(l, t)| text(*l, t)).collect(),
        summaries: summaries.iter().map(|(l, t)| text(Some(l), t)).collect(),
        ..WebMetadata::default()
    }
}

#[test]
fn web_fills_what_the_files_do_not_say() {
    let mangadex = WebMetadata {
        authors: vec![author("Oda")],
        genres: vec!["Action".into()],
        year: Some(1997),
        status: Some("ongoing".into()),
        links: vec!["https://mangadex.org/title/x".into()],
        ..web(
            "x",
            &[(Some("ja-ro"), "One Piece"), (Some("ja"), "ワンピース")],
            &[("en", "A boy."), ("fr", "Un garçon.")],
        )
    };
    let anilist = WebMetadata {
        tags: vec!["Pirates".into()],
        links: vec!["https://anilist.co/manga/1".into()],
        ..web(
            "1",
            &[(Some("en"), "One Piece"), (None, "Wan Pīsu")],
            &[("en", "A boy (AniList).")],
        )
    };
    let meta = with_web(
        SeriesMetadata::default(),
        "one-piece",
        &[mangadex.clone(), anilist.clone()],
        "fr",
        false,
    );
    // Pas de titre français : le romaji de la première source ; le résumé en français.
    assert_eq!(meta.title.as_deref(), Some("One Piece"));
    assert_eq!(meta.alt_titles, ["ワンピース", "Wan Pīsu"]);
    assert_eq!(meta.summary.as_deref(), Some("Un garçon."));
    assert_eq!(
        (meta.authors.len(), meta.genres.clone(), meta.tags.clone()),
        (1, vec!["Action".into()], vec!["Pirates".into()])
    );
    assert_eq!(
        (meta.year, meta.publication_status),
        (Some(1997), Some(PublicationStatus::Ongoing))
    );
    assert_eq!(meta.links.len(), 2);
    // En anglais, AniList d'abord : son résumé ; le titre reste celui du dossier (aucune fiche ne
    // donne le même nom).
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[anilist, mangadex.clone()],
        "en",
        false,
    );
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref()),
        (None, Some("A boy (AniList)."))
    );
    // Les fichiers l'emportent ; le titre du web reste cherchable.
    let local = SeriesMetadata {
        title: Some("Mon titre".into()),
        summary: Some("Le mien.".into()),
        year: Some(2000),
        ..SeriesMetadata::default()
    };
    let meta = with_web(local, "x", &[mangadex], "fr", false);
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref(), meta.year),
        (Some("Mon titre"), Some("Le mien."), Some(2000))
    );
    assert!(meta.alt_titles.contains(&"One Piece".to_owned()));
    // Sans fiche, rien ne change ; sans romaji ni langue voulue, le premier titre et le premier résumé.
    assert_eq!(
        with_web(SeriesMetadata::default(), "x", &[], "fr", false),
        SeriesMetadata::default()
    );
    let odd = with_web(
        SeriesMetadata::default(),
        "x",
        &[web("y", &[(None, "Titre")], &[("es", "Resumen.")])],
        "fr",
        false,
    );
    assert_eq!(
        (odd.title.as_deref(), odd.summary.as_deref()),
        (None, Some("Resumen."))
    );
}

#[test]
fn the_language_comes_before_the_main_source() {
    let anilist = web(
        "1",
        &[(Some("en"), "One Piece"), (None, "Wan Pīsu")],
        &[("en", "A boy (AniList).")],
    );
    let mangadex = web(
        "x",
        &[(Some("fr"), "One Piece (fr)")],
        &[("fr", "Un garçon.")],
    );
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[anilist.clone(), mangadex.clone()],
        "fr",
        true,
    );
    // Le français de MangaDex passe avant l'anglais de la source principale (lot 10.2).
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref()),
        (None, Some("Un garçon."))
    );
    // À langue égale, la source principale l'emporte.
    let english = web(
        "y",
        &[(Some("en"), "One Piece (y)")],
        &[("en", "A boy (y).")],
    );
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[anilist.clone(), english],
        "fr",
        true,
    );
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref()),
        (None, Some("A boy (AniList)."))
    );
    // Sans source principale, la langue de l'interface l'emporte aussi.
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[anilist, mangadex.clone()],
        "fr",
        false,
    );
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref()),
        (None, Some("Un garçon."))
    );
    // La source principale sans résumé (interface en anglais) : son titre, et le résumé d'une autre
    // fiche en repli, faute d'anglais nulle part.
    let silent = web("2", &[(Some("en"), "Silent")], &[]);
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[silent, mangadex],
        "en",
        true,
    );
    assert_eq!(
        (meta.title.as_deref(), meta.summary.as_deref()),
        (None, Some("Un garçon."))
    );
}

/// Cagaster (vu sur une vraie bibliothèque) : chaque `ComicInfo.xml` date les tomes du
/// 1970-01-21, date vide d'un outil mal convertie. Ni la série ni le tome ne la gardent :
/// l'année vient alors des sources web, et une vraie date de 1970 hors janvier reste.
#[test]
fn a_january_1970_date_is_a_blank_and_is_ignored() {
    let mut blank = comic("Cagaster", "Tome 1", "1970");
    blank.1.release_date = Some("1970-01-21".into());
    assert_eq!(
        from_locals("Cagaster", std::slice::from_ref(&blank)).year,
        None
    );
    assert_eq!(volume_from_locals(&[blank]).release_date, None);
    let real = comic("Ashita no Joe", "Tome 1", "1970");
    assert_eq!(
        from_locals("x", std::slice::from_ref(&real)).year,
        Some(1970)
    );
    assert_eq!(
        volume_from_locals(&[real]).release_date.as_deref(),
        Some("1970-07")
    );
    let web = WebMetadata {
        year: Some(2014),
        ..WebMetadata::default()
    };
    let mut blank = comic("Cagaster", "Tome 1", "1970");
    blank.1.release_date = Some("1970-01".into());
    let meta = with_web(
        from_locals("Cagaster", &[blank]),
        "Cagaster",
        &[web],
        "fr",
        false,
    );
    assert_eq!(meta.year, Some(2014));
}

/// Retour d'usage (2026-11-21) : le titre affiché est celui du dossier, jamais un autre titre du
/// web (romaji « Appurushīdo » pour le dossier Appleseed, traduction mot à mot de MangaDex pour
/// Gintama) ; une fiche n'en donne que l'écriture, quand elle a le même nom.
#[test]
fn the_folder_name_beats_the_web_titles() {
    let fiche = web(
        "1",
        &[
            (Some("ja-ro"), "Appurushīdo"),
            (Some("en"), "Appleseed"),
            (Some("fr"), "Graine de pomme"),
        ],
        &[("fr", "Résumé.")],
    );
    let meta = with_web(
        SeriesMetadata::default(),
        "Appleseed",
        std::slice::from_ref(&fiche),
        "fr",
        false,
    );
    // Pas de titre : le nom du dossier s'affiche ; les titres de la fiche restent cherchables.
    assert_eq!(meta.title, None);
    assert!(meta.alt_titles.contains(&"Appurushīdo".to_owned()));
    assert!(meta.alt_titles.contains(&"Graine de pomme".to_owned()));
    // Dossier en minuscules avec tirets : la fiche corrige l'écriture.
    let one = web("2", &[(Some("en"), "One Piece")], &[]);
    let meta = with_web(SeriesMetadata::default(), "one-piece", &[one], "fr", false);
    assert_eq!(meta.title.as_deref(), Some("One Piece"));
    // Un nom écrit avec des majuscules est gardé, même si une fiche l'écrit autrement.
    let city = web("4", &[(Some("en"), "CITY HUNTER")], &[]);
    let meta = with_web(
        SeriesMetadata::default(),
        "City Hunter",
        &[city],
        "fr",
        false,
    );
    assert_eq!(meta.title, None);
    // Aucun titre de même nom : pas de titre (le dossier s'affiche), même avec un titre français.
    let meta = with_web(
        SeriesMetadata::default(),
        "Gintama",
        &[web("3", &[(Some("fr"), "L'Âme d'argent")], &[])],
        "fr",
        false,
    );
    assert_eq!(meta.title, None);
}

#[test]
fn the_official_volume_count_comes_from_the_files_then_the_sources() {
    let web = |count: Option<u32>| WebMetadata {
        volume_count: count,
        ..WebMetadata::default()
    };
    // Le plus grand `<Count>` des fichiers : celui du T01 date d'avant les tomes suivants.
    let count = |n: u32| {
        let (source, mut meta) = comic("One Piece", "r", "1997");
        meta.volume_count = Some(n);
        (source, meta)
    };
    assert_eq!(
        from_locals("x", &[count(3), count(12), count(10)]).volume_count,
        Some(12)
    );
    // Le fichier passe devant ; sans lui, la première fiche qui le connaît.
    let local = from_locals("x", &[count(12)]);
    let meta = with_web(local, "x", &[web(None), web(Some(15))], "fr", false);
    assert_eq!(meta.volume_count, Some(12));
    let meta = with_web(
        SeriesMetadata::default(),
        "x",
        &[web(None), web(Some(15)), web(Some(9))],
        "fr",
        false,
    );
    assert_eq!(meta.volume_count, Some(15));
    // Champ éditable comme les autres.
    let mut edited_meta = Edited::new();
    apply(
        &mut edited_meta,
        edited(json!({ "volume_count": 20 })),
        SERIES_FIELDS,
    )
    .unwrap();
    assert_eq!(overlay(meta, &edited_meta).unwrap().volume_count, Some(20));
}
