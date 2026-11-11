use super::*;

fn volume(content: &[u8]) -> VolumeId {
    VolumeId::from_reader(content).unwrap()
}

#[test]
fn volume_id_is_frozen() {
    // Valeur de référence : BLAKE3("shiori").
    assert_eq!(
        volume(b"shiori").to_string(),
        blake3::hash(b"shiori").to_hex().as_str()
    );
}

#[test]
fn page_id_is_frozen() {
    // Instantané : si ce test casse, toutes les URLs de pages existantes cassent.
    let id = PageId::derive(&volume(b"shiori"), b"001.jpg");
    assert_eq!(id.to_string(), FROZEN_PAGE_ID);
}
const FROZEN_PAGE_ID: &str = "012ed07bf4eedd2ff59719acbe4b6f5f";

#[test]
fn page_id_depends_on_volume_and_raw_name() {
    let a = volume(b"a");
    let b = volume(b"b");
    let base = PageId::derive(&a, b"001.jpg");
    assert_eq!(base, PageId::derive(&a, b"001.jpg"));
    assert_ne!(base, PageId::derive(&b, b"001.jpg"));
    assert_ne!(base, PageId::derive(&a, b"002.jpg"));
    // Le séparateur empêche qu'un nom « déborde » sur le volume.
    assert_ne!(base, PageId::derive(&a, b"001.jpg\0"));
}

#[test]
fn from_file_matches_from_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.cbz");
    std::fs::write(&path, b"contenu").unwrap();
    assert_eq!(VolumeId::from_file(&path).unwrap(), volume(b"contenu"));
}

#[test]
fn hex_round_trip_and_strict_parsing() {
    let v = volume(b"x");
    assert_eq!(v.to_string().parse::<VolumeId>().unwrap(), v);
    let p = PageId::derive(&v, b"p");
    assert_eq!(p.to_string().parse::<PageId>().unwrap(), p);

    assert!(p.to_string().to_uppercase().parse::<PageId>().is_err());
    assert!("abc".parse::<PageId>().is_err());
    assert!(v.to_string().parse::<PageId>().is_err());
    assert!("zz".repeat(16).parse::<PageId>().is_err());
}
