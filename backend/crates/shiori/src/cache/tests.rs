use std::time::Duration;

use shiori_core::VolumeId;

use super::*;

fn page(n: u8) -> PageId {
    let volume = VolumeId::from_reader(&b"cache"[..]).unwrap();
    PageId::derive(&volume, &[n])
}

fn age(images: &Images, id: &PageId, width: u32, secs: u64) {
    let path = images.path(id, width, Format::Jpeg);
    let file = File::options().write(true).open(path).unwrap();
    let old = SystemTime::now() - Duration::from_secs(secs);
    file.set_times(FileTimes::new().set_modified(old)).unwrap();
}

#[test]
fn put_then_get_and_usage_survives_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    assert_eq!(images.get(&page(1), 320, Format::Jpeg), None);
    images.put(&page(1), 320, Format::Jpeg, b"abc").unwrap();
    images.put(&page(1), 320, Format::Avif, b"de").unwrap();
    assert_eq!(
        images.get(&page(1), 320, Format::Jpeg).as_deref(),
        Some(&b"abc"[..])
    );
    assert_eq!(images.get(&page(1), 640, Format::Jpeg), None);
    assert_eq!(images.used(), 5);
    // Aucun fichier temporaire ne reste.
    let names: Vec<String> = files(&dir.path().join("pages"))
        .iter()
        .map(|f| f.0.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    let reopened = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    assert_eq!(reopened.used(), 5);
}

#[test]
fn eviction_removes_the_least_recently_read_first() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), 35).unwrap();
    for n in 0..3 {
        images.put(&page(n), 320, Format::Jpeg, &[n; 10]).unwrap();
    }
    // Trois fichiers de 10 octets tiennent sous 35. Les dates d'écriture sont trop proches :
    // on les fixe pour un ordre net, puis on relit le plus ancien.
    age(&images, &page(0), 320, 300);
    age(&images, &page(1), 320, 200);
    age(&images, &page(2), 320, 100);
    assert!(images.get(&page(0), 320, Format::Jpeg).is_some());
    // 40 octets > 35 : éviction jusqu'à 27 (90 %, arrondi), les moins récemment lus d'abord.
    images.put(&page(3), 320, Format::Jpeg, &[3; 10]).unwrap();
    assert_eq!(
        images.get(&page(1), 320, Format::Jpeg),
        None,
        "le plus ancien part"
    );
    assert_eq!(images.get(&page(2), 320, Format::Jpeg), None);
    assert!(
        images.get(&page(0), 320, Format::Jpeg).is_some(),
        "relu : gardé"
    );
    assert!(images.get(&page(3), 320, Format::Jpeg).is_some());
    assert_eq!(images.used(), 20);
}

#[test]
fn write_errors_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    // Un dossier à la place du fichier de destination : le renommage échoue.
    std::fs::create_dir_all(images.path(&page(9), 320, Format::Jpeg)).unwrap();
    assert!(images.put(&page(9), 320, Format::Jpeg, b"x").is_err());
    let leftovers = files(&dir.path().join("pages"));
    assert!(
        leftovers
            .iter()
            .all(|f| !f.0.to_string_lossy().ends_with(".tmp"))
    );
    // Un fichier à la place du dossier du cache : ouverture impossible.
    let file = dir.path().join("fichier");
    std::fs::write(&file, b"").unwrap();
    assert!(Images::open(file, 1).is_err());
}
