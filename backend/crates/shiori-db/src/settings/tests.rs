use super::*;

#[tokio::test]
async fn language_is_unset_then_chosen_then_reset() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    assert_eq!(
        db.settings().await.unwrap(),
        Settings {
            language: None,
            page_turn: PageTurn::Reading
        }
    );
    let en = db.set_language(Some(Language::En)).await.unwrap();
    assert_eq!(en.language, Some(Language::En));
    assert_eq!(db.settings().await.unwrap(), en);
    assert_eq!(db.set_language(None).await.unwrap().language, None);
}

#[tokio::test]
async fn page_turn_is_saved_without_touching_the_language() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    db.set_language(Some(Language::Fr)).await.unwrap();
    let saved = db.set_page_turn(PageTurn::Rtl).await.unwrap();
    assert_eq!(
        (saved.language, saved.page_turn),
        (Some(Language::Fr), PageTurn::Rtl)
    );
}
