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
    let names: Vec<String> = units(dir.path())
        .iter()
        .map(|u| u.path.file_name().unwrap().to_string_lossy().into_owned())
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
    let leftovers = units(dir.path());
    assert!(
        leftovers
            .iter()
            .all(|u| !u.path.to_string_lossy().ends_with(".tmp"))
    );
    // Un fichier à la place du dossier du cache : ouverture impossible.
    let file = dir.path().join("fichier");
    std::fs::write(&file, b"").unwrap();
    assert!(Images::open(file, 1).is_err());
}

fn volume(n: u8) -> VolumeId {
    VolumeId::from_reader(&[n][..]).unwrap()
}

/// Extraction factice : `sizes[i]` octets pour l'entrée `i`.
fn fake(sizes: &[usize]) -> impl FnOnce(&Path) -> io::Result<()> {
    move |dir: &Path| {
        for (i, &n) in sizes.iter().enumerate() {
            std::fs::write(dir.join(i.to_string()), vec![i as u8; n])?;
        }
        Ok(())
    }
}

fn never(_: &Path) -> io::Result<()> {
    panic!("le tome est déjà extrait")
}

fn age_dir(images: &Images, volume: &VolumeId, secs: u64) {
    let dir = images.dir.join("extract").join(volume.to_string());
    let old = SystemTime::now() - Duration::from_secs(secs);
    for file in std::fs::read_dir(dir).unwrap() {
        let file = File::options()
            .write(true)
            .open(file.unwrap().path())
            .unwrap();
        file.set_times(FileTimes::new().set_modified(old)).unwrap();
    }
}

#[test]
fn a_volume_is_extracted_once_then_read_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    let v = volume(1);
    assert_eq!(images.extracted(&v, 0, 3, fake(&[3, 2])).unwrap(), [0; 3]);
    assert_eq!(images.extracted(&v, 1, 2, never).unwrap(), [1; 2]);
    assert_eq!(images.used(), 5);

    // Taille différente du scan, ou entrée absente de l'extraction : erreur, sans réextraire.
    let short = images.extracted(&v, 1, 9, never).unwrap_err();
    assert_eq!(short.kind(), io::ErrorKind::InvalidData);
    let absent = images.extracted(&v, 7, 1, never).unwrap_err();
    assert_eq!(absent.kind(), io::ErrorKind::NotFound);

    // Restes d'un lancement interrompu : effacés à l'ouverture, jamais comptés.
    let extract = dir.path().join("extract");
    std::fs::create_dir_all(extract.join("abc.tmp")).unwrap();
    std::fs::create_dir_all(extract.join("abc.del")).unwrap();
    let reopened = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    assert_eq!(reopened.used(), 5);
    let names: Vec<_> = std::fs::read_dir(&extract)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, [v.to_string()]);
}

#[test]
fn a_failed_extraction_leaves_nothing_behind() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), u64::MAX).unwrap();
    let v = volume(2);
    let failed = images.extracted(&v, 0, 1, |dir: &Path| {
        std::fs::write(dir.join("0"), b"x")?;
        Err(io::Error::other("archive illisible"))
    });
    assert_eq!(failed.unwrap_err().to_string(), "archive illisible");
    assert_eq!(
        std::fs::read_dir(dir.path().join("extract"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(images.used(), 0);
    // Une nouvelle demande réessaie.
    assert_eq!(images.extracted(&v, 0, 1, fake(&[1])).unwrap(), [0]);
}

/// Les tomes extraits et les rendus partagent le plafond ; le dernier tome lu n'est jamais
/// évincé, même plus gros que le plafond à lui seul.
#[test]
fn extracted_volumes_share_the_cap_and_the_last_one_stays() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), 25).unwrap();
    images.put(&page(0), 320, Format::Jpeg, &[0; 10]).unwrap();
    age(&images, &page(0), 320, 300);
    images.extracted(&volume(1), 0, 10, fake(&[10])).unwrap();
    age_dir(&images, &volume(1), 200);
    // 10 + 10 + 30 > 25 : tout part sauf le tome qu'on vient de lire.
    images.extracted(&volume(2), 0, 30, fake(&[30])).unwrap();
    assert_eq!(images.get(&page(0), 320, Format::Jpeg), None);
    assert!(
        !dir.path()
            .join("extract")
            .join(volume(1).to_string())
            .exists()
    );
    assert_eq!(images.extracted(&volume(2), 0, 30, never).unwrap(), [0; 30]);
    assert_eq!(images.used(), 30);
}

/// Un tome qu'on ne peut pas renommer (page ouverte sous Windows) reste entier et compté.
#[test]
fn a_volume_that_cannot_be_moved_is_kept_whole() {
    let dir = tempfile::tempdir().unwrap();
    let images = Images::open(dir.path().to_path_buf(), 15).unwrap();
    images.extracted(&volume(1), 0, 10, fake(&[10])).unwrap();
    age_dir(&images, &volume(1), 200);
    // Un fichier à la place du dossier de suppression : le renommage échoue.
    let blocked = dir
        .path()
        .join("extract")
        .join(format!("{}.del", volume(1)));
    std::fs::write(&blocked, b"").unwrap();
    images.extracted(&volume(2), 0, 10, fake(&[10])).unwrap();
    assert_eq!(images.extracted(&volume(1), 0, 10, never).unwrap(), [0; 10]);
    assert_eq!(images.used(), 20);
}
