use super::*;

#[test]
fn archive_errors_map_to_codes() {
    use std::io;
    let cases = [
        (
            ArchiveError::Truncated("x".into()),
            IssueCode::ArchiveTruncated,
        ),
        (ArchiveError::Encrypted, IssueCode::EncryptedEntry),
        (
            ArchiveError::UnsupportedCompression("x".into()),
            IssueCode::UnsupportedCompression,
        ),
        (ArchiveError::CrcMismatch, IssueCode::CrcMismatch),
        (ArchiveError::SizeMismatch, IssueCode::SizeMismatch),
        (ArchiveError::EntryTooLarge(1), IssueCode::EntryTooLarge),
        (
            ArchiveError::Unreadable("x".into()),
            IssueCode::ArchiveUnreadable,
        ),
        (ArchiveError::NoSuchEntry(0), IssueCode::ArchiveUnreadable),
        (
            ArchiveError::UnsupportedArchive("7z"),
            IssueCode::UnsupportedArchive,
        ),
        (
            ArchiveError::Io(io::Error::other("x")),
            IssueCode::ArchiveUnreadable,
        ),
    ];
    for (err, code) in cases {
        assert_eq!(issue_for(&err), code, "{err}");
    }
}

#[test]
fn known_junk_is_ignored_wherever_it_is() {
    for name in [
        "ComicInfo.xml",
        "comicinfo.XML",
        "sub/Thumbs.db",
        ".DS_Store",
        "__MACOSX/a/._01.jpg",
        "x\\__MACOSX\\y",
        "Desktop.ini",
        "release.NFO",
        "OEBPS/text/part0001.xhtml",
        "index.html",
        "stylesheet.css",
        "content.opf",
        "toc.ncx",
    ] {
        assert!(is_ignored(name), "{name}");
    }
    for name in [
        "01.jpg",
        "comicinfo.xml.jpg",
        "MACOSX/01.jpg",
        "html/01.jpg",
    ] {
        assert!(!is_ignored(name), "{name}");
    }
}

#[test]
fn extensions_are_case_insensitive() {
    assert_eq!(extension("A/B.JPEG").as_deref(), Some("jpeg"));
    assert_eq!(extension("sans-extension"), None);
    assert_eq!(media_for_extension("jpeg"), Some("image/jpeg"));
    assert_eq!(media_for_extension("avif"), Some("image/avif"));
    assert_eq!(media_for_extension("gif"), Some("image/gif"));
    assert_eq!(media_for_extension("tif"), None);
}

#[test]
fn rewriting_a_file_changes_its_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.cbz");
    fs::write(&path, b"version 1").unwrap();
    let before = stamp(&fs::metadata(&path).unwrap());
    assert_eq!(before, stamp(&fs::metadata(&path).unwrap()));
    fs::write(&path, b"version 2, plus longue").unwrap();
    assert_ne!(before, stamp(&fs::metadata(&path).unwrap()));
}

#[test]
fn a_panic_becomes_an_issue_of_that_file() {
    let r = guarded(Path::new("piege.cbz"), || panic!("décodeur en vrac"));
    assert_eq!(r.status, Status::Error);
    assert_eq!(r.issues[0].code, IssueCode::ArchiveUnreadable);
    assert!(r.issues[0].message.contains("décodeur en vrac"));
    assert!(r.volume_id.is_none() && r.pages.is_empty());

    let owned = guarded(Path::new("piege.cbz"), || {
        panic!("{}", String::from("message dynamique"))
    });
    assert!(owned.issues[0].message.contains("message dynamique"));
}

#[test]
fn decoding_stops_at_the_memory_limit() {
    let page = shiori_testkit::png(64, 64, [10, 20, 30]);
    assert!(decode_image(&page).is_ok());
    // 64×64 en RGB : 12 Kio, au-delà d'un plafond de 1 Kio.
    let err = decode_within(&page, 1024).unwrap_err();
    assert!(matches!(err, image::ImageError::Limits(_)), "{err}");
    assert!(decode_image(b"pas une image").is_err());
}

#[test]
fn a_jpeg_with_a_short_comment_still_gives_its_size() {
    let bytes = shiori_testkit::jpeg_short_comment(8, 12, [1, 2, 3]);
    assert!(
        imagesize::blob_size(&bytes).is_err(),
        "la fixture doit reproduire le défaut"
    );
    assert_eq!(dimensions(&bytes), Some((8, 12)));
    assert_eq!(dimensions(b"pas une image"), None);
}
