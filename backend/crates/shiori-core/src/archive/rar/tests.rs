use std::collections::BTreeMap;

use shiori_testkit::{RAR4, RAR5, fixtures, page_color, png};

use super::*;

/// Contenu attendu de chaque entrée du faux manga, par nom.
fn expected() -> BTreeMap<String, Vec<u8>> {
    let mut out: BTreeMap<String, Vec<u8>> = (0..fixtures::RAR_PAGES)
        .map(|i| {
            let name = format!("ch{}/{:02}.png", i / 3 + 1, i % 3 + 1);
            (name, png(60, 90, page_color(i)))
        })
        .collect();
    out.insert("ComicInfo.xml".into(), b"<ComicInfo/>".to_vec());
    out
}

fn each(
    archive: &mut RarArchive,
    indices: &[usize],
) -> Vec<(usize, Result<Vec<u8>, ArchiveError>)> {
    let mut out = Vec::new();
    archive.read_each(indices, &mut |i, r| out.push((i, r)));
    out
}

#[test]
fn every_kind_lists_and_reads_its_entries() {
    let dir = tempfile::tempdir().unwrap();
    for (version, solid) in [(RAR5, false), (RAR5, true), (RAR4, false), (RAR4, true)] {
        let name = format!("{version:?}-{solid}.cbr");
        let path = fixtures::rar(dir.path(), &name, version, solid);
        let mut archive = RarArchive::open(&path).unwrap();
        assert_eq!(archive.is_solid(), solid, "{name}");
        assert!(!archive.is_multivolume());
        assert!(archive.duplicates().unwrap().is_empty());
        let entries = archive.entries().to_vec();
        let want = expected();
        let names: BTreeMap<_, _> = entries.iter().map(|e| (e.name.clone(), e.index)).collect();
        assert_eq!(
            names.keys().collect::<Vec<_>>(),
            want.keys().collect::<Vec<_>>(),
            "{name}"
        );
        for e in &entries {
            assert_eq!(e.kind, EntryKind::File);
            assert_eq!(
                e.name_raw.len(),
                e.name.len(),
                "noms ASCII : octets bruts = nom"
            );
            assert_ne!(e.crc32, 0);
            assert_eq!(e.size, want[&e.name].len() as u64);
        }
        // Accès direct, puis une seule passe : mêmes contenus, ordre de l'archive.
        for e in &entries {
            assert_eq!(
                archive.read_entry(e.index).unwrap(),
                want[&e.name],
                "{name}"
            );
        }
        let streamed = std::io::Read::bytes(archive.open_entry(0).unwrap()).count();
        assert_eq!(streamed, entries[0].size as usize);
        let all: Vec<usize> = entries.iter().map(|e| e.index).rev().collect();
        let read = each(&mut archive, &all);
        let order: Vec<usize> = read.iter().map(|r| r.0).collect();
        assert_eq!(order, (0..entries.len()).collect::<Vec<_>>());
        for (i, bytes) in read {
            assert_eq!(bytes.unwrap(), want[&entries[i].name]);
        }
        // Une partie seulement, et rien du tout.
        let read = each(&mut archive, &[2]);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].1.as_ref().unwrap(), &want[&entries[2].name]);
        assert!(each(&mut archive, &[]).is_empty());
        let missing = each(&mut archive, &[99]);
        assert!(matches!(
            missing.as_slice(),
            [(99, Err(ArchiveError::NoSuchEntry(99)))]
        ));
        assert!(matches!(
            archive.read_entry(99),
            Err(ArchiveError::NoSuchEntry(99))
        ));
    }
}

#[test]
fn duplicate_names_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut rar = rars::Builder::new(RAR5).allow_duplicate_names(true);
    for content in [&b"un"[..], b"un", b"deux"] {
        rar.add_bytes(b"a.png".to_vec(), content.to_vec(), None, None)
            .unwrap();
    }
    rar.add_bytes(b"b.png".to_vec(), b"x".to_vec(), None, None)
        .unwrap();
    rar.add_bytes(b"b.png".to_vec(), b"x".to_vec(), None, None)
        .unwrap();
    let path = dir.path().join("dups.cbr");
    rar.write_to_path(&path, None).unwrap();
    let archive = RarArchive::open(&path).unwrap();
    assert_eq!(archive.entries().len(), 5);
    let dups = archive.duplicates().unwrap();
    let found: Vec<_> = dups
        .iter()
        .map(|d| (d.name.as_str(), d.identical))
        .collect();
    assert_eq!(found, [("a.png", false), ("b.png", true)]);
}

#[test]
fn directories_and_encrypted_entries() {
    let dir = tempfile::tempdir().unwrap();
    let mut rar = rars::Builder::new(RAR5).password(Some(b"secret".to_vec()));
    rar.add_directory(b"ch1".to_vec(), None, None).unwrap();
    rar.add_bytes(b"ch1/01.png".to_vec(), png(8, 8, [1, 2, 3]), None, None)
        .unwrap();
    let path = dir.path().join("chiffre.cbr");
    rar.write_to_path(&path, None).unwrap();
    let mut archive = RarArchive::open(&path).unwrap();
    let kinds: Vec<_> = archive
        .entries()
        .iter()
        .map(|e| (e.kind, e.encrypted))
        .collect();
    assert_eq!(kinds, [(EntryKind::Dir, true), (EntryKind::File, true)]);
    assert!(matches!(
        archive.read_entry(1),
        Err(ArchiveError::Encrypted)
    ));
    let read = each(&mut archive, &[1]);
    assert!(matches!(read[0], (1, Err(ArchiveError::Encrypted))));
}

/// Un octet changé dans une page stockée : CRC faux pour elle seule, les autres se lisent
/// (archive non solid).
#[test]
fn a_bad_crc_spoils_only_its_page_when_not_solid() {
    let dir = tempfile::tempdir().unwrap();
    let pages: Vec<Vec<u8>> = (0..3).map(|i| vec![b'a' + i as u8; 64]).collect();
    let mut rar = rars::Builder::new(RAR5).store(true);
    for (i, p) in pages.iter().enumerate() {
        rar.add_bytes(format!("{i}.png").into_bytes(), p.clone(), None, None)
            .unwrap();
    }
    let path = dir.path().join("crc.cbr");
    rar.write_to_path(&path, None).unwrap();
    shiori_testkit::replace_all(&path, &[b'b'; 64], &[b'B'; 64]);
    let mut archive = RarArchive::open(&path).unwrap();
    assert!(matches!(
        archive.read_entry(1),
        Err(ArchiveError::CrcMismatch)
    ));
    let read = each(&mut archive, &[0, 1, 2]);
    assert_eq!(read[0].1.as_ref().unwrap(), &pages[0]);
    assert!(matches!(read[1], (1, Err(ArchiveError::CrcMismatch))));
    assert_eq!(read[2].1.as_ref().unwrap(), &pages[2]);
}

/// Archive solid abîmée au milieu : les premières pages se lisent, les suivantes sont en
/// erreur explicite, jamais absentes ni remplacées.
#[test]
fn a_damaged_solid_archive_fails_from_the_damage() {
    let dir = tempfile::tempdir().unwrap();
    let (path, pages) = fixtures::damaged_solid_rar(dir.path());
    let mut archive = RarArchive::open(&path).unwrap();
    let indices: Vec<usize> = (0..archive.entries().len()).collect();
    let read = each(&mut archive, &indices);
    assert_eq!(read.len(), pages.len(), "chaque page a un résultat");
    let first_bad = read.iter().position(|r| r.1.is_err()).unwrap();
    assert!(first_bad > 0);
    assert!(read[first_bad..].iter().all(|r| r.1.is_err()));
    for (i, r) in &read[..first_bad] {
        assert_eq!(r.as_ref().unwrap(), &pages[*i]);
    }
    let later = read.last().unwrap().1.as_ref().unwrap_err().to_string();
    assert!(later.contains("interrompu plus tôt"), "{later}");
}

#[test]
fn unreadable_files() {
    let dir = tempfile::tempdir().unwrap();
    let tiny = dir.path().join("tiny.cbr");
    std::fs::write(&tiny, b"Rar!").unwrap();
    assert!(RarArchive::open(&tiny).is_err());
    // Coupé : `rars` lit tous les en-têtes à l'ouverture et voit qu'il en manque.
    let cut = fixtures::rar(dir.path(), "coupe.cbr", RAR5, true);
    let full = std::fs::read(&cut).unwrap();
    std::fs::write(&cut, &full[..full.len() / 2]).unwrap();
    assert!(matches!(
        RarArchive::open(&cut),
        Err(ArchiveError::Truncated(_))
    ));
    let missing = dir.path().join("absent.cbr");
    assert!(matches!(
        RarArchive::open(&missing),
        Err(ArchiveError::Io(_))
    ));
}

/// Une entrée annoncée plus grosse que `MAX_ENTRY_SIZE` n'est jamais lue en mémoire ; dans
/// une archive solid, son contenu est décodé sans être gardé pour lire les suivantes.
#[test]
fn oversized_entries_are_refused_without_reading_them() {
    let dir = tempfile::tempdir().unwrap();
    for solid in [false, true] {
        let path = fixtures::rar(dir.path(), "gros.cbr", RAR5, solid);
        let mut archive = RarArchive::open(&path).unwrap();
        archive.entries[0].size = MAX_ENTRY_SIZE + 1;
        assert!(matches!(
            archive.read_entry(0),
            Err(ArchiveError::EntryTooLarge(_))
        ));
        let read = each(&mut archive, &[0, 1]);
        assert!(matches!(read[0], (0, Err(ArchiveError::EntryTooLarge(_)))));
        assert!(read[1].1.is_ok(), "solid = {solid}");
    }
}

/// Un membre qui produit plus d'octets que sa taille déclarée : refusé, jamais tronqué en
/// silence ni gardé en mémoire au-delà de cette taille.
#[test]
fn a_member_longer_than_declared_is_a_size_mismatch() {
    let buffer = Rc::new(RefCell::new(Capped {
        bytes: Vec::new(),
        max: 4,
        overflow: false,
    }));
    let mut writer = Shared(buffer.clone());
    writer.write_all(b"abc").unwrap();
    assert!(writer.write_all(b"de").is_err());
    writer.flush().unwrap();
    let mut seen = Vec::new();
    let mut visit =
        |i, r: Result<Vec<u8>, ArchiveError>| seen.push((i, r.map_err(|e| e.to_string())));
    let mut stream = Stream {
        visit: &mut visit,
        seen: 0,
        current: Some((3, buffer)),
        visited: BTreeSet::new(),
    };
    // `rars` remonte le refus du tampon en erreur d'E/S : c'est quand même `size_mismatch`.
    stream.flush(Some(ArchiveError::Io(std::io::Error::other(
        "écriture refusée",
    ))));
    // Plus court que déclaré : refusé aussi.
    stream.current = Some((
        4,
        Rc::new(RefCell::new(Capped {
            bytes: vec![1],
            max: 2,
            overflow: false,
        })),
    ));
    stream.flush(None);
    stream.flush(None); // rien en cours : sans effet
    drop(stream);
    let mismatch = Err(ArchiveError::SizeMismatch.to_string());
    assert_eq!(seen, [(3, mismatch.clone()), (4, mismatch)]);
}

#[test]
fn rars_errors_map_to_scan_categories() {
    let eof = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "fin");
    let cases = [
        (rars::Error::TooShort, "Truncated"),
        (
            rars::Error::TotalOutputLimitExceeded {
                limit: 1,
                used: 0,
                required: 2,
            },
            "SizeMismatch",
        ),
        (rars::Error::NeedPassword, "Encrypted"),
        (rars::Error::WrongPasswordOrCorruptData, "Encrypted"),
        (rars::Error::UnsupportedSignature, "UnsupportedCompression"),
        (rars::Error::from(eof), "Truncated"),
        (rars::Error::from(std::io::Error::other("disque")), "Io"),
        (rars::Error::InvalidHeader("en-tête"), "Unreadable"),
        (
            rars::Error::InvalidHeader("x").at_entry(b"a.png".to_vec(), "lecture"),
            "Unreadable",
        ),
    ];
    for (e, want) in cases {
        let got = format!("{:?}", error(&e));
        assert!(got.starts_with(want), "{e} → {got}");
    }
}
