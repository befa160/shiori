use super::*;

const COMIC_INFO: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Romance Dawn</Title>
  <Series>One Piece</Series>
  <Number>1</Number>
  <AlternateSeries>ワンピース</AlternateSeries>
  <Summary>  Luffy part en mer.  </Summary>
  <Year>1997</Year>
  <Month>12</Month>
  <Day>-1</Day>
  <Writer>Eiichiro Oda</Writer>
  <Penciller>Eiichiro Oda, Assistant</Penciller>
  <Colorist></Colorist>
  <Publisher>Glénat</Publisher>
  <Genre>Aventure, Action, aventure</Genre>
  <Web>https://a.example https://b.example</Web>
  <LanguageISO>fr</LanguageISO>
  <Manga>YesAndRightToLeft</Manga>
  <AgeRating>Unknown</AgeRating>
  <Pages><Page Image="0" Type="FrontCover"/></Pages>
</ComicInfo>"#;

#[test]
fn comic_info_is_normalized() {
    let meta = parse(MetadataSource::Comicinfo, COMIC_INFO.as_bytes()).unwrap();
    assert_eq!(
        meta,
        LocalMetadata {
            series: Some("One Piece".into()),
            title: Some("Romance Dawn".into()),
            number: Some("1".into()),
            alt_titles: vec!["ワンピース".into()],
            summary: Some("Luffy part en mer.".into()),
            authors: vec![
                Author {
                    name: "Eiichiro Oda".into(),
                    story: true,
                    art: true
                },
                Author {
                    name: "Assistant".into(),
                    story: false,
                    art: true
                },
            ],
            genres: vec!["Aventure".into(), "Action".into()],
            tags: vec![],
            release_date: Some("1997-12".into()),
            publisher: Some("Glénat".into()),
            language: Some("fr".into()),
            age_rating: None,
            status: None,
            right_to_left: Some(true),
            links: vec!["https://a.example".into(), "https://b.example".into()],
        }
    );
}

#[test]
fn bom_and_utf16_are_read() {
    let xml = "<ComicInfo><Series>Été</Series><Manga>No</Manga></ComicInfo>";
    let mut utf8 = b"\xEF\xBB\xBF".to_vec();
    utf8.extend_from_slice(xml.as_bytes());
    let mut le = vec![0xFF, 0xFE];
    le.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    let mut be = vec![0xFE, 0xFF];
    be.extend(xml.encode_utf16().flat_map(u16::to_be_bytes));
    for bytes in [utf8, le, be] {
        let meta = parse(MetadataSource::Comicinfo, &bytes).unwrap();
        assert_eq!(meta.series.as_deref(), Some("Été"));
        assert_eq!(meta.right_to_left, Some(false));
    }
}

#[test]
fn broken_files_are_errors_never_panics() {
    for (source, bytes) in [
        (
            MetadataSource::Comicinfo,
            &b"<ComicInfo><Series>x</Serie>"[..],
        ),
        (MetadataSource::Comicinfo, b"\xFF\xFE\x00\xD8"),
        (MetadataSource::Comicinfo, b"\xC3\x28"),
        (MetadataSource::IndexJson, b"{\"title\": 3}"),
        (MetadataSource::IndexJson, b"[1, 2]"),
    ] {
        assert!(parse(source, bytes).is_err(), "{bytes:?}");
    }
}

#[test]
fn dates_keep_what_is_known() {
    let s = |v: &str| Some(v.to_owned());
    assert_eq!(date(s("2020"), s("2"), s("3")), s("2020-02-03"));
    assert_eq!(date(s("2020"), s("13"), s("3")), s("2020"));
    assert_eq!(date(s("2020"), None, None), s("2020"));
    assert_eq!(date(s("-1"), s("2"), None), None);
    assert_eq!(date(None, None, None), None);
}

#[test]
fn index_json_is_normalized() {
    // Forme du fichier réel (application de téléchargement), allégée.
    let json = r#"{
        "id": 1323383001932260791, "title": "Leviathan", "alt_titles": ["", "Léviathan"],
        "author": "lee kyung tak / noh mi young",
        "authors": ["lee kyung tak / noh mi young"],
        "description": "La saison des pluies sans fin a commencé.",
        "state": "FINISHED", "source": "WEBTOONS_FR",
        "public_url": "https://www.webtoons.com/fr/sf/leviathan/list?title_no=7320",
        "tags": [{"key": "sf", "title": "SF"}],
        "chapters": {"1": {"number": 1, "name": "Ep. 1"}}
    }"#;
    let meta = parse(MetadataSource::IndexJson, json.as_bytes()).unwrap();
    let unknown = |name: &str| Author {
        name: name.into(),
        story: false,
        art: false,
    };
    assert_eq!(
        meta,
        LocalMetadata {
            series: Some("Leviathan".into()),
            alt_titles: vec!["Léviathan".into()],
            summary: Some("La saison des pluies sans fin a commencé.".into()),
            authors: vec![unknown("lee kyung tak"), unknown("noh mi young")],
            tags: vec!["SF".into()],
            status: Some("completed".into()),
            links: vec!["https://www.webtoons.com/fr/sf/leviathan/list?title_no=7320".into()],
            ..LocalMetadata::default()
        }
    );
    let states = [
        ("ONGOING", "ongoing"),
        ("PAUSED", "hiatus"),
        ("ABANDONED", "cancelled"),
    ];
    for (state, status) in states {
        let json = format!(r#"{{"state": "{state}"}}"#);
        let meta = parse(MetadataSource::IndexJson, json.as_bytes()).unwrap();
        assert_eq!(meta.status.as_deref(), Some(status));
    }
    let meta = parse(MetadataSource::IndexJson, br#"{"state": "?"}"#).unwrap();
    assert_eq!(meta, LocalMetadata::default());
}

#[test]
fn only_root_files_are_metadata() {
    assert_eq!(
        MetadataSource::of_entry("ComicInfo.XML"),
        Some(MetadataSource::Comicinfo)
    );
    assert_eq!(
        MetadataSource::of_entry("index.json"),
        Some(MetadataSource::IndexJson)
    );
    assert_eq!(MetadataSource::of_entry("tome/ComicInfo.xml"), None);
    assert_eq!(MetadataSource::of_entry("tome\\index.json"), None);
    assert_eq!(MetadataSource::of_entry("001.jpg"), None);
}

#[test]
fn rows_saved_before_a_new_field_still_load() {
    let meta: LocalMetadata = serde_json::from_str(r#"{"series": "x"}"#).unwrap();
    assert_eq!(meta.series.as_deref(), Some("x"));
}
