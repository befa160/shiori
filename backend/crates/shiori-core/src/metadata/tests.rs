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
  <Web>https://a.example javascript:alert(1) HTTP://b.example</Web>
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
            links: vec!["https://a.example".into(), "HTTP://b.example".into()],
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

#[test]
fn a_title_written_in_web_is_not_a_link() {
    // Vu dans un vrai tome (Shangri-la Frontier T12) : un titre romaji dans `<Web>`.
    let xml = "<ComicInfo><Web>Shangri-La Frontier: Kusoge Hunter</Web></ComicInfo>";
    let meta = parse(MetadataSource::Comicinfo, xml.as_bytes()).unwrap();
    assert!(meta.links.is_empty());
    let meta = parse(
        MetadataSource::IndexJson,
        br#"{"public_url": "javascript:x"}"#,
    )
    .unwrap();
    assert!(meta.links.is_empty());
}

#[test]
fn pdf_info_gives_authors_summary_and_tags_only() {
    // « Yûji Kaku ; Yuji Kaku » en UTF-16BE avec BOM, comme l'écrit Calibre hors ASCII.
    let mut author = vec![0xFE, 0xFF];
    for unit in "Yûji Kaku ; yûji kaku; ".encode_utf16() {
        author.extend(unit.to_be_bytes());
    }
    let meta = pdf_info(
        Some(&author),
        Some(b"  R\xe9sum\xe9 du tome  "),
        Some(b"shonen, Ninja; ninja ,"),
    )
    .unwrap();
    assert_eq!(
        meta,
        LocalMetadata {
            authors: vec![Author {
                name: "Yûji Kaku".into(),
                story: true,
                art: true,
            }],
            // PDFDocEncoding (ici Latin-1), pas de l'UTF-8.
            summary: Some("Résumé du tome".into()),
            tags: vec!["shonen".into(), "Ninja".into()],
            ..LocalMetadata::default()
        }
    );
    assert_eq!(pdf_info(None, Some(b"   "), Some(b" ; ,")), None);
    // Calibre joint les auteurs par « & » ; un « & » collé reste dans le nom.
    let two = pdf_info(
        Some(b"Tsugumi Ohba & Takeshi Obata;Studio&Co"),
        None,
        Some(b"Sword & Sorcery"),
    )
    .unwrap();
    let names: Vec<_> = two.authors.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Tsugumi Ohba", "Takeshi Obata", "Studio&Co"]);
    assert_eq!(two.tags, ["Sword & Sorcery"]);
    // Un champ démesuré est ignoré.
    let huge = vec![b'a'; MAX_METADATA_SIZE as usize + 1];
    assert_eq!(pdf_info(None, Some(&huge), None), None);
    assert!(parse(MetadataSource::Pdf, b"").is_err());
}

#[test]
fn a_malformed_comic_info_keeps_its_readable_fields() {
    // `<Day>` mal placé et non fermé, `<Series>` en double, entité et CDATA.
    let xml = "<ComicInfo><Series>Tom &amp; Jerry</Series><Series>Autre</Series>\
               <Year>2001<Day>3</Year><Summary><![CDATA[a < b]]></Summary></ComicInfo>";
    assert!(parse(MetadataSource::Comicinfo, xml.as_bytes()).is_err());
    let meta = salvage_comic_info(xml.as_bytes()).unwrap();
    assert_eq!(meta.series.as_deref(), Some("Tom & Jerry"));
    assert_eq!(meta.summary.as_deref(), Some("a < b"));
    assert_eq!(meta.release_date, None, "<Year> mal fermé : illisible");
    for nothing in [
        &b"<ComicInfo>\n  <Pages/>\n</ComicInfo>"[..],
        b"\xC3\x28",
        b"",
    ] {
        assert_eq!(salvage_comic_info(nothing), None, "{nothing:?}");
    }
}
