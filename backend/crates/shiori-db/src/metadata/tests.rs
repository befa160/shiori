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
            // Le nom du dossier n'est pas répété ; doublons sans casse retirés.
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

        let [s] = &db.all_series().await.unwrap()[..] else {
            panic!()
        };
        let id = s.id;
        assert_eq!(s.name, "One Piece");
        // « ONE PIECE » du fichier = le nom du dossier, à la casse près : pas un autre titre.
        assert!(s.alt_titles.is_empty());
        assert_eq!(s.authors, ["Eiichiro Oda"]);
        let volume = db.series_detail(id).await.unwrap().unwrap().volumes[0]
            .volume_id
            .clone();
        let volume: shiori_core::VolumeId = volume.parse().unwrap();
        let pages = db.pages_of(&volume).await.unwrap().unwrap();

        // Titre et couverture édités : la liste suit ; le rescan ne les touche pas.
        let edit =
            patch(json!({"title": "Wan Pīsu", "cover_page_id": pages[1].id, "summary": null}));
        assert!(db.edit_series(id, edit).await.unwrap());
        pass().await;
        let [s] = &db.all_series().await.unwrap()[..] else {
            panic!()
        };
        assert_eq!(
            (s.name.as_str(), s.cover_page_id.as_deref()),
            ("Wan Pīsu", Some(pages[1].id.as_str()))
        );
        let detail = db.series_detail(id).await.unwrap().unwrap();
        assert_eq!(detail.name, "Wan Pīsu");
        assert_eq!(detail.metadata.summary, None);
        assert_eq!(detail.locked, ["cover_page_id", "summary", "title"]);

        // Rendu aux sources : la valeur du fichier revient.
        assert!(
            db.edit_series(id, patch(json!({"unlock": ["summary", "title"]})))
                .await
                .unwrap()
        );
        let detail = db.series_detail(id).await.unwrap().unwrap();
        assert_eq!(
            (detail.name.as_str(), detail.metadata.summary.as_deref()),
            ("One Piece", Some("Luffy."))
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
        let v = db.volume_detail(&volume).await.unwrap().unwrap();
        assert_eq!(v.metadata.title.as_deref(), Some("Romance Dawn"));
        assert!(
            db.edit_volume(&volume, patch(json!({"title": "Tome 1"})))
                .await
                .unwrap()
        );
        let v = db.volume_detail(&volume).await.unwrap().unwrap();
        assert_eq!(
            (v.metadata.title.as_deref(), v.locked.as_slice()),
            (Some("Tome 1"), &["title".to_owned()][..])
        );
        assert!(matches!(
            db.edit_volume(&volume, patch(json!({"genres": []}))).await,
            Err(crate::EditError::Invalid(_))
        ));
        let unknown: shiori_core::VolumeId = "0".repeat(64).parse().unwrap();
        assert!(!db.edit_volume(&unknown, patch(json!({}))).await.unwrap());

        // Le fichier change : nouvelles pages, la couverture choisie disparaît.
        std::fs::remove_file(&file).unwrap();
        write_zip(
            &folder,
            "One Piece T01.cbz",
            &[("01.png", &png(4, 6, page_color(1)), DEFLATED)],
        );
        pass().await;
        let [s] = &db.all_series().await.unwrap()[..] else {
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
        let id = db.all_series().await.unwrap()[0].id;
        for round in 0..10 {
            let (a, b) = tokio::join!(
                db.edit_series(id, patch(json!({"summary": format!("r{round}")}))),
                db.edit_series(id, patch(json!({"genres": [format!("g{round}")]}))),
            );
            assert!(a.unwrap() && b.unwrap());
            let m = db.series_detail(id).await.unwrap().unwrap().metadata;
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
        let a = db.series_detail(1).await.unwrap().unwrap();
        assert_eq!(a.publication_status, Some(PublicationStatus::Hiatus));
        assert_eq!(a.locked, ["publication_status"]);
        let b = db.series_detail(2).await.unwrap().unwrap();
        assert_eq!((b.publication_status, b.locked.len()), (None, 0));
    }
}
