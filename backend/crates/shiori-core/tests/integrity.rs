//! Scan d'intégrité : un test par fixture cassée, un test par invariant.

use std::path::Path;

use shiori_core::{
    Archive, CbzArchive, FileReport, IssueCode, PageStatus, ScanOptions, Severity, Status,
    scan_file,
};
use shiori_testkit::{DEFLATED, fixtures, page_color, png, write_zip};

fn scan(path: &Path) -> FileReport {
    scan_file(path, ScanOptions::default())
}

fn codes(report: &FileReport) -> Vec<IssueCode> {
    report.issues.iter().map(|i| i.code).collect()
}

fn names(report: &FileReport) -> Vec<&str> {
    report.pages.iter().map(|p| p.entry_name.as_str()).collect()
}

fn issue_entry(report: &FileReport, code: IssueCode) -> Option<&str> {
    report
        .issues
        .iter()
        .find(|i| i.code == code)
        .and_then(|i| i.entry.as_deref())
}

// --- Fixtures saines -------------------------------------------------------------

#[test]
fn webtoon_is_clean_and_fully_described() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::webtoon(dir.path()));
    assert_eq!(r.status, Status::Ok, "{:?}", r.issues);
    assert!(r.issues.is_empty());
    assert_eq!(r.pages.len(), fixtures::WEBTOON_PAGES);
    for (i, p) in r.pages.iter().enumerate() {
        assert_eq!(p.ordinal, i);
        assert_eq!(p.entry_name, format!("{:03}.png", i + 1));
        assert_eq!((p.width, p.height), (Some(32), Some(2400)));
        assert_eq!(p.media_type, Some("image/png"));
        assert_eq!(p.status, PageStatus::Ok);
    }
}

#[test]
fn manga_has_cover_spreads_and_back_cover() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::manga(dir.path()));
    // ComicInfo.xml n'est qu'une information : le tome reste « ok ».
    assert_eq!(r.status, Status::Ok);
    assert_eq!(codes(&r), [IssueCode::IgnoredEntry]);
    assert_eq!(r.pages.len(), fixtures::MANGA_PAGES);
    assert_eq!(r.pages[0].media_type, Some("image/jpeg"));
    assert_eq!(r.pages[4].width, Some(1200));
    assert_eq!(r.pages[7].width, Some(1200));
    assert_eq!(r.pages.last().unwrap().entry_name, "011 - 4e.png");
}

// --- Invariants ------------------------------------------------------------------

/// Le bug « 4ème de couverture répétée » est impossible : chaque page relue par son
/// index d'archive est exactement l'image attendue à cet ordinal.
#[test]
fn every_page_is_the_right_image() {
    let dir = tempfile::tempdir().unwrap();
    for path in [fixtures::webtoon(dir.path()), fixtures::manga(dir.path())] {
        let r = scan(&path);
        let mut archive = CbzArchive::open(&path).unwrap();
        for p in &r.pages {
            let bytes = archive.read_entry(p.archive_index).unwrap();
            let pixel = image::load_from_memory(&bytes).unwrap().to_rgb8()[(0, 0)].0;
            let expected = page_color(p.ordinal);
            let close = pixel.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 8);
            assert!(
                close,
                "{}: page {} = {pixel:?}, attendu {expected:?}",
                path.display(),
                p.ordinal
            );
            assert_eq!(archive.entries()[p.archive_index].crc32, p.crc32);
        }
    }
}

#[test]
fn ids_survive_rescans_copies_and_renames() {
    let dir = tempfile::tempdir().unwrap();
    let original = fixtures::manga(dir.path());
    let moved_dir = dir.path().join("ailleurs");
    std::fs::create_dir(&moved_dir).unwrap();
    let moved = moved_dir.join("Mon manga T01 (renommé).cbz");
    std::fs::copy(&original, &moved).unwrap();

    let a = scan(&original);
    for b in [scan(&original), scan(&moved)] {
        assert_eq!(a.volume_id, b.volume_id);
        let ids = |r: &FileReport| {
            r.pages
                .iter()
                .map(|p| (p.ordinal, p.id))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&a), ids(&b));
    }
}

#[test]
fn changed_content_changes_every_id() {
    let dir = tempfile::tempdir().unwrap();
    let one = png(8, 8, [1, 2, 3]);
    let two = png(8, 8, [9, 9, 9]);
    let v1 = write_zip(dir.path(), "v1.cbz", &[("001.png", &one, DEFLATED)]);
    let v2 = write_zip(dir.path(), "v2.cbz", &[("001.png", &two, DEFLATED)]);
    let (a, b) = (scan(&v1), scan(&v2));
    assert_ne!(a.volume_id, b.volume_id);
    assert_ne!(a.pages[0].id, b.pages[0].id);
}

#[test]
fn scanning_never_modifies_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixtures::manga(dir.path());
    let before = std::fs::read(&path).unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms.clone()).unwrap();

    let r = scan_file(&path, ScanOptions { deep: true });
    assert_eq!(r.status, Status::Ok);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);

    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false); // sinon Windows refuse de nettoyer le dossier temporaire
    std::fs::set_permissions(&path, perms).unwrap();
}

// --- Archives cassées : un code attendu par situation ----------------------------

#[test]
fn unpadded_names_are_read_in_natural_order() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::unpadded(dir.path()));
    assert_eq!(r.status, Status::Warning);
    assert_eq!(codes(&r), [IssueCode::ArchiveOrderDiffers]);
    let expected: Vec<String> = (1..=11).map(|n| format!("{n}.png")).collect();
    assert_eq!(names(&r), expected);
}

#[test]
fn messy_archive_reports_subfolders_parasites_and_ignored_entries() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::messy(dir.path()));
    assert_eq!(r.status, Status::Warning);
    assert_eq!(names(&r), ["ch1/01.png", "ch2/01.png", "ch2/02.png"]);
    assert_eq!(
        issue_entry(&r, IssueCode::NonImageEntry),
        Some("lisez-moi.txt")
    );
    assert_eq!(
        codes(&r)
            .iter()
            .filter(|c| **c == IssueCode::IgnoredEntry)
            .count(),
        2
    );
    assert!(codes(&r).contains(&IssueCode::Subdirectories));
}

#[test]
fn identical_duplicates_keep_one_page() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::duplicates(dir.path(), true));
    assert_eq!(r.status, Status::Warning);
    assert_eq!(
        issue_entry(&r, IssueCode::DuplicateEntryIdentical),
        Some("01.png")
    );
    assert_eq!(names(&r), ["01.png", "02.png"]);
    assert!(r.pages.iter().all(|p| p.status == PageStatus::Ok));
}

#[test]
fn conflicting_duplicates_break_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::duplicates(dir.path(), false));
    assert_eq!(r.status, Status::Error);
    assert_eq!(
        issue_entry(&r, IssueCode::DuplicateEntryConflict),
        Some("01.png")
    );
    assert_eq!(r.pages[0].status, PageStatus::Broken);
    assert_eq!(r.pages[1].status, PageStatus::Ok);
}

#[test]
fn bad_crc_breaks_only_that_page_and_keeps_its_place() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::bad_crc(dir.path()));
    assert_eq!(r.status, Status::Error);
    assert_eq!(issue_entry(&r, IssueCode::CrcMismatch), Some("02.png"));
    assert_eq!(names(&r), ["01.png", "02.png"]);
    assert_eq!(r.pages[0].status, PageStatus::Ok);
    assert_eq!(r.pages[1].status, PageStatus::Broken);
}

#[test]
fn truncated_archive_is_reported_as_such() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::truncated(dir.path()));
    assert_eq!(r.status, Status::Error);
    assert_eq!(codes(&r), [IssueCode::ArchiveTruncated]);
    assert!(r.volume_id.is_some());
    assert!(r.pages.is_empty());
}

#[test]
fn encrypted_entry_is_a_broken_page() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::encrypted(dir.path()));
    assert_eq!(r.status, Status::Error);
    assert_eq!(codes(&r), [IssueCode::EncryptedEntry]);
    assert_eq!(r.pages[0].status, PageStatus::Broken);
}

#[test]
fn odd_images_are_classified() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::odd_images(dir.path()));
    assert_eq!(r.status, Status::Error);
    assert_eq!(names(&r), ["01.jpg", "02.png", "04.png", "05.webp"]);
    assert_eq!(
        issue_entry(&r, IssueCode::ExtensionMismatch),
        Some("01.jpg")
    );
    assert_eq!(r.pages[0].media_type, Some("image/png"));
    assert_eq!(r.pages[0].status, PageStatus::Ok);
    assert_eq!(issue_entry(&r, IssueCode::ImageUndecodable), Some("02.png"));
    assert_eq!(r.pages[1].status, PageStatus::Broken);
    assert_eq!(issue_entry(&r, IssueCode::UnsupportedImage), Some("03.gif"));
    let undecodable: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i.code == IssueCode::ImageUndecodable)
        .filter_map(|i| i.entry.as_deref())
        .collect();
    assert_eq!(undecodable, ["02.png", "05.webp"]);
    assert_eq!(r.pages[3].status, PageStatus::Broken);
}

#[test]
fn every_supported_format_becomes_a_page() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixtures::all_formats(dir.path());
    for deep in [false, true] {
        let r = scan_file(&path, ScanOptions { deep });
        assert_eq!(r.status, Status::Ok, "deep={deep} {:?}", r.issues);
        let media: Vec<_> = r.pages.iter().map(|p| p.media_type.unwrap()).collect();
        assert_eq!(
            media,
            ["image/jpeg", "image/png", "image/webp", "image/avif"]
        );
        assert!(
            r.pages
                .iter()
                .all(|p| (p.width, p.height) == (Some(8), Some(12)))
        );
    }
}

#[test]
fn symlinks_are_not_pages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lien.cbz");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("01.png", opts).unwrap();
    std::io::Write::write_all(&mut zip, &png(8, 8, [0, 0, 0])).unwrap();
    zip.add_symlink("02.png", "01.png", opts).unwrap();
    zip.finish().unwrap();
    let r = scan(&path);
    assert_eq!(names(&r), ["01.png"]);
    assert_eq!(issue_entry(&r, IssueCode::NonImageEntry), Some("02.png"));
}

#[test]
fn corrupt_image_body_is_caught_only_in_deep_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixtures::corrupt_body(dir.path());
    assert_eq!(scan(&path).status, Status::Ok);

    let deep = scan_file(&path, ScanOptions { deep: true });
    assert_eq!(deep.status, Status::Error);
    assert_eq!(
        issue_entry(&deep, IssueCode::ImageUndecodable),
        Some("01.jpg")
    );
    assert_eq!(deep.pages[0].status, PageStatus::Broken);
}

#[test]
fn archive_without_images_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::no_images(dir.path()));
    assert_eq!(r.status, Status::Error);
    assert_eq!(codes(&r), [IssueCode::NonImageEntry, IssueCode::NoPages]);
}

#[test]
fn unreadable_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::not_a_zip(dir.path()));
    assert_eq!(codes(&r), [IssueCode::ArchiveUnreadable]);
    assert!(r.volume_id.is_some());

    let missing = scan(&dir.path().join("absent.cbz"));
    assert_eq!(missing.status, Status::Error);
    assert_eq!(codes(&missing), [IssueCode::ArchiveUnreadable]);
    assert!(missing.volume_id.is_none());
}

#[test]
fn severities_follow_codes() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::odd_images(dir.path()));
    assert!(r.issues.iter().all(|i| i.severity == i.code.severity()));
    assert_eq!(IssueCode::IgnoredEntry.severity(), Severity::Info);
}

#[test]
fn report_serializes_ids_as_hex_and_hides_raw_names() {
    let dir = tempfile::tempdir().unwrap();
    let r = scan(&fixtures::manga(dir.path()));
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["volume_id"].as_str().unwrap().len(), 64);
    assert_eq!(json["pages"][0]["id"].as_str().unwrap().len(), 32);
    assert_eq!(json["pages"][0]["status"], "ok");
    assert_eq!(json["issues"][0]["code"], "ignored_entry");
    assert!(json["pages"][0].get("entry_name_raw").is_none());
}
