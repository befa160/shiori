use serde_json::json;
use shiori_meta::{Text, WebMetadata};

use super::*;
use crate::{Language, LibraryKind};

async fn setup() -> (tempfile::TempDir, Db, i64, i64) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let root = dir.path().join("m");
    std::fs::create_dir(&root).unwrap();
    let library = db
        .create_library("M", &root, LibraryKind::Manga)
        .await
        .unwrap();
    for (folder, name) in [("one-piece", "one-piece"), ("akira", "Akira")] {
        sqlx::query(
            "INSERT INTO series (library_id, folder, name, created_at) VALUES (?, ?, ?, '')",
        )
        .bind(library.id)
        .bind(folder)
        .bind(name)
        .execute(db.pool())
        .await
        .unwrap();
    }
    db.refresh_metadata(None).await.unwrap();
    let ids: Vec<i64> = db
        .all_series()
        .await
        .unwrap()
        .iter()
        .map(|s| s.id)
        .collect();
    let id = |name: &str| ids[if name == "Akira" { 0 } else { 1 }];
    (dir, db, id("one-piece"), id("Akira"))
}

fn fiche(id: &str, title: &str, lang: &str) -> WebMetadata {
    WebMetadata {
        id: id.into(),
        titles: vec![Text {
            lang: Some(lang.into()),
            text: title.into(),
        }],
        summaries: vec![Text {
            lang: Some(lang.into()),
            text: format!("Résumé de {title}"),
        }],
        cover_url: Some("https://exemple/c.jpg".into()),
        ..WebMetadata::default()
    }
}

async fn series(db: &Db, id: i64) -> crate::SeriesDetail {
    db.series_detail(id).await.unwrap().unwrap()
}

async fn left(db: &Db, source: SourceId) -> Vec<i64> {
    db.to_identify(source, None)
        .await
        .unwrap()
        .iter()
        .map(|t| t.series_id)
        .collect()
}

fn linked(id: &str, title: &str, lang: &str) -> Outcome {
    Outcome::Linked {
        external_id: id.into(),
        matched_by: MatchedBy::Auto,
        data: Box::new(fiche(id, title, lang)),
    }
}

#[tokio::test]
async fn linked_sources_feed_the_merged_view_in_the_chosen_order() {
    let (_dir, db, one, _) = setup().await;
    db.record_source(
        one,
        SourceId::Anilist,
        linked("30013", "One Piece (AniList)", "en"),
    )
    .await
    .unwrap();
    db.record_source(
        one,
        SourceId::Mangadex,
        linked("a1c7", "One Piece (MangaDex)", "fr"),
    )
    .await
    .unwrap();
    // Français par défaut : MangaDex d'abord.
    let detail = series(&db, one).await;
    assert_eq!(detail.name, "One Piece (MangaDex)");
    assert_eq!(
        detail.metadata.summary.as_deref(),
        Some("Résumé de One Piece (MangaDex)")
    );
    assert_eq!(detail.sources.len(), 2);
    assert_eq!(detail.sources[0].matched_by, Some(MatchedBy::Auto));
    // En anglais : AniList d'abord, recalculé aussitôt.
    db.set_language(Some(Language::En)).await.unwrap();
    assert_eq!(series(&db, one).await.name, "One Piece (AniList)");
    // Ordre choisi : seulement MangaDex ; la fiche AniList ne compte plus.
    db.set_metadata_sources(Some(vec![SourceId::Mangadex]))
        .await
        .unwrap();
    let detail = series(&db, one).await;
    assert_eq!(detail.name, "One Piece (MangaDex)");
    assert!(
        !detail
            .metadata
            .alt_titles
            .contains(&"One Piece (AniList)".to_owned())
    );
    // Délier rend la place.
    assert!(db.unlink_source(one, SourceId::Mangadex).await.unwrap());
    assert!(!db.unlink_source(one, SourceId::Mangadex).await.unwrap());
    assert_eq!(series(&db, one).await.name, "one-piece");
}

#[tokio::test]
async fn candidates_failures_and_what_is_left_to_identify() {
    let (_dir, db, one, akira) = setup().await;
    assert_eq!(left(&db, SourceId::Anilist).await, [one, akira]);
    let names = db.to_identify(SourceId::Anilist, Some(one)).await.unwrap();
    assert_eq!(
        names,
        [ToIdentify {
            series_id: one,
            names: vec!["one-piece".into()]
        }]
    );

    // Plusieurs fiches possibles : à confirmer, la couverture n'est pas montrée.
    db.record_source(
        one,
        SourceId::Anilist,
        Outcome::Candidates(vec![fiche("1", "A", "en"), fiche("2", "B", "en")]),
    )
    .await
    .unwrap();
    // Échec : retenté plus tard ; rien trouvé : pas retenté.
    db.record_source(akira, SourceId::Anilist, Outcome::Failed("offline"))
        .await
        .unwrap();
    db.record_source(akira, SourceId::Mangadex, Outcome::Candidates(vec![]))
        .await
        .unwrap();
    assert_eq!(left(&db, SourceId::Anilist).await, [akira]);
    assert_eq!(left(&db, SourceId::Mangadex).await, [one]);
    let states = db.source_states(one).await.unwrap();
    assert_eq!(
        (
            states[0].candidates.len(),
            states[0].candidates[0].cover_url.clone()
        ),
        (2, None)
    );
    let issues = db.source_issues().await.unwrap();
    assert_eq!(
        issues
            .iter()
            .map(|i| (i.series_id, i.source, i.candidates, i.last_error.as_deref()))
            .collect::<Vec<_>>(),
        [
            (akira, SourceId::Anilist, 0, Some("offline")),
            (one, SourceId::Anilist, 2, None)
        ]
    );

    // Une source liée qui échoue à la relecture garde sa fiche.
    db.record_source(akira, SourceId::Anilist, linked("7", "AKIRA", "en"))
        .await
        .unwrap();
    db.record_source(akira, SourceId::Anilist, Outcome::Failed("rate_limited"))
        .await
        .unwrap();
    let detail = series(&db, akira).await;
    assert_eq!(detail.name, "AKIRA");
    assert_eq!(
        detail.sources[0].last_error.as_deref(),
        Some("rate_limited")
    );
    assert_eq!(
        db.linked_sources(akira).await.unwrap(),
        [(SourceId::Anilist, "7".to_owned())]
    );
    // Liée : plus à identifier, même après un échec de relecture.
    assert_eq!(left(&db, SourceId::Anilist).await, Vec::<i64>::new());
}

#[tokio::test]
async fn an_edited_field_beats_the_web_and_settings_round_trip() {
    let (_dir, db, one, _) = setup().await;
    let edit = json!({ "publication_status": "hiatus" })
        .as_object()
        .unwrap()
        .clone();
    db.edit_series(one, edit).await.unwrap();
    let ongoing = Outcome::Linked {
        external_id: "1".into(),
        matched_by: MatchedBy::User,
        data: Box::new(WebMetadata {
            status: Some("ongoing".into()),
            ..fiche("1", "One Piece", "en")
        }),
    };
    db.record_source(one, SourceId::Anilist, ongoing)
        .await
        .unwrap();
    let detail = series(&db, one).await;
    assert_eq!(
        detail.publication_status,
        Some(crate::PublicationStatus::Hiatus)
    );
    assert_eq!(detail.sources[0].matched_by, Some(MatchedBy::User));

    let settings = db.settings().await.unwrap();
    assert!(!settings.metadata_online, "désactivé par défaut");
    assert_eq!(
        settings.source_order(),
        [SourceId::Mangadex, SourceId::Anilist]
    );
    assert_eq!(settings.lang(), "fr");
    assert!(db.set_metadata_online(true).await.unwrap().metadata_online);
    let custom = db
        .set_metadata_sources(Some(vec![SourceId::Anilist]))
        .await
        .unwrap();
    assert_eq!(custom.source_order(), [SourceId::Anilist]);
    assert_eq!(
        db.set_metadata_sources(None)
            .await
            .unwrap()
            .metadata_sources,
        None
    );
}

#[tokio::test]
async fn a_series_gone_meanwhile_is_skipped() {
    let (_dir, db, one, _) = setup().await;
    sqlx::query("DELETE FROM series WHERE id = ?")
        .bind(one)
        .execute(db.pool())
        .await
        .unwrap();
    for outcome in [
        linked("1", "X", "en"),
        Outcome::Candidates(vec![]),
        Outcome::Failed("offline"),
    ] {
        db.record_source(one, SourceId::Anilist, outcome)
            .await
            .unwrap();
    }
    assert!(db.source_states(one).await.unwrap().is_empty());
}
