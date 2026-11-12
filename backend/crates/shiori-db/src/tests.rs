use super::*;
use shiori_core::{PageId, PageStatus};

/// Rapport synthétique de `n` pages, sans archive : mesure le coût de la persistance seule.
fn synthetic(n: usize) -> FileReport {
    let volume = VolumeId::from_reader(&b"synthetique"[..]).unwrap();
    let pages = (0..n)
        .map(|i| {
            let name = format!("00000000_{i:08}.jpg");
            Page {
                id: PageId::derive(&volume, name.as_bytes()),
                ordinal: i,
                entry_name_raw: name.clone().into_bytes(),
                entry_name: name,
                archive_index: n - 1 - i,
                media_type: Some("image/jpeg"),
                width: Some(800),
                height: Some(1280),
                compressed_size: 150_000,
                size: 151_000,
                crc32: i as u32,
                status: PageStatus::Ok,
            }
        })
        .collect();
    FileReport {
        path: "synthetique.cbz".into(),
        volume_id: Some(volume),
        format: "cbz",
        size: 2_000_000_000,
        status: Status::Ok,
        pages,
        issues: Vec::new(),
    }
}

/// Le webtoon réel de l'essai compte 12 760 pages : l'écriture doit rester rapide et
/// le relu identique à l'écrit.
#[tokio::test]
async fn saves_and_reloads_a_huge_volume() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let report = synthetic(12_760);

    let t = std::time::Instant::now();
    db.save(run, "/synthetique.cbz", Some((1, 1)), &report)
        .await
        .unwrap();
    let first = t.elapsed();
    let t = std::time::Instant::now();
    db.save(run, "/synthetique.cbz", Some((1, 1)), &report)
        .await
        .unwrap();
    eprintln!(
        "12 760 pages : écriture {first:?}, réécriture {:?}",
        t.elapsed()
    );

    let volume = report.volume_id.unwrap();
    let loaded = db
        .load_report(&volume, Path::new("synthetique.cbz"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded, report);
}

#[test]
fn enum_names_come_from_serde() {
    assert_eq!(
        to_db(&IssueCode::ArchiveOrderDiffers),
        "archive_order_differs"
    );
    assert_eq!(
        from_db::<IssueCode>("crc_mismatch").unwrap(),
        IssueCode::CrcMismatch
    );
    assert!(from_db::<IssueCode>("inconnu").is_err());
    assert_eq!(media_type(Some("image/avif")), Some("image/avif"));
    assert_eq!(media_type(Some("image/gif")), None);
    assert_eq!(media_type(None), None);
}

#[test]
fn location_keys_are_absolute_with_forward_slashes() {
    let key = location_key(Path::new("relatif/tome.cbz"));
    assert!(key.ends_with("relatif/tome.cbz"), "{key}");
    assert!(Path::new(&key.replace('/', std::path::MAIN_SEPARATOR_STR)).is_absolute());
}
