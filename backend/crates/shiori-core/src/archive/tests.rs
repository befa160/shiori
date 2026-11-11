use super::*;
use std::io::Write;
use std::path::PathBuf;
use zip::write::SimpleFileOptions;

fn write_zip(dir: &Path, name: &str, files: &[(&str, &[u8], CompressionMethod)]) -> PathBuf {
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (entry, content, method) in files {
        if entry.ends_with('/') {
            zip.add_directory(*entry, SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(
                *entry,
                SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
            zip.write_all(content).unwrap();
        }
    }
    zip.finish().unwrap();
    path
}

fn replace_all(path: &Path, from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut bytes = std::fs::read(path).unwrap();
    let mut found = 0;
    for i in 0..=bytes.len() - from.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            found += 1;
        }
    }
    assert!(found > 0, "motif introuvable");
    std::fs::write(path, bytes).unwrap();
}

const STORED: CompressionMethod = CompressionMethod::Stored;
const DEFLATED: CompressionMethod = CompressionMethod::Deflated;

#[test]
fn lists_entries_in_directory_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        dir.path(),
        "a.cbz",
        &[
            ("10.jpg", b"dix", DEFLATED),
            ("ch1/", b"", STORED),
            ("2.jpg", b"deux", STORED),
        ],
    );
    let archive = CbzArchive::open(&path).unwrap();
    let e = archive.entries();
    assert_eq!(e.len(), 3);
    assert_eq!(
        e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["10.jpg", "ch1/", "2.jpg"]
    );
    assert_eq!(e[0].index, 0);
    assert_eq!(e[0].name_raw, b"10.jpg");
    assert_eq!(e[0].size, 3);
    assert_eq!(e[0].crc32, crc32(b"dix"));
    assert_eq!(e[0].compression, "Deflated");
    assert!(e[0].compression_supported && !e[0].encrypted);
    assert_eq!(e[1].kind, EntryKind::Dir);
    assert_eq!(e[2].kind, EntryKind::File);
    assert!(!archive.has_duplicate_names());
}

fn crc32(data: &[u8]) -> u32 {
    // CRC-32/ISO-HDLC, calculé à part pour ne pas dépendre de `zip`.
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[test]
fn reads_entries_by_index() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![7u8; 100_000];
    let path = write_zip(
        dir.path(),
        "a.cbz",
        &[("a.jpg", &big, DEFLATED), ("b.jpg", b"bee", STORED)],
    );
    let mut archive = CbzArchive::open(&path).unwrap();
    assert_eq!(archive.read_entry(1).unwrap(), b"bee");
    assert_eq!(archive.read_entry(0).unwrap(), big);
    assert!(matches!(
        archive.read_entry(2),
        Err(ArchiveError::NoSuchEntry(2))
    ));
}

#[test]
fn detects_crc_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        dir.path(),
        "a.cbz",
        &[("a.jpg", b"contenu-original", STORED)],
    );
    replace_all(&path, b"contenu-original", b"contenu-altere!!");
    let mut archive = CbzArchive::open(&path).unwrap();
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::CrcMismatch)
    ));
}

#[test]
fn rejects_non_zip_and_truncated_files() {
    let dir = tempfile::tempdir().unwrap();
    let junk = dir.path().join("junk.cbz");
    std::fs::write(&junk, b"ceci n'est pas un zip").unwrap();
    assert!(matches!(
        CbzArchive::open(&junk),
        Err(ArchiveError::Unreadable(_))
    ));

    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"aaaa", STORED)]);
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();
    assert!(matches!(
        CbzArchive::open(&path),
        Err(ArchiveError::Unreadable(_) | ArchiveError::Truncated(_))
    ));
}

#[test]
fn duplicate_names_are_flagged() {
    // `zip` fusionne les entrées de même nom : on vérifie qu'il le signale,
    // l'analyse fine des doublons relève des contrôles d'intégrité.
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(
        dir.path(),
        "a.cbz",
        &[("a.jpg", b"un", STORED), ("b.jpg", b"deux", STORED)],
    );
    replace_all(&path, b"b.jpg", b"a.jpg");
    let archive = CbzArchive::open(&path).unwrap();
    assert!(archive.has_duplicate_names());
    assert_eq!(archive.entries().len(), 1);
}

#[test]
fn opening_does_not_modify_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"aaaa", DEFLATED)]);
    let before = std::fs::read(&path).unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();

    let mut archive = CbzArchive::open(&path).unwrap();
    archive.read_entry(0).unwrap();
    drop(archive);

    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);

    // Windows refuse de supprimer un fichier en lecture seule : on rétablit les droits
    // pour que le dossier temporaire soit nettoyé.
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&path, perms).unwrap();
}

/// Applique `f` au champ u16 situé à `local_off` dans chaque en-tête local et à
/// `central_off` dans chaque en-tête de l'annuaire central.
fn patch_header_u16(path: &Path, local_off: usize, central_off: usize, f: impl Fn(u16) -> u16) {
    let mut bytes = std::fs::read(path).unwrap();
    let mut patched = 0;
    for i in 0..bytes.len().saturating_sub(4) {
        let off = match &bytes[i..i + 4] {
            b"PK\x03\x04" => local_off,
            b"PK\x01\x02" => central_off,
            _ => continue,
        };
        let at = i + off;
        let v = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        bytes[at..at + 2].copy_from_slice(&f(v).to_le_bytes());
        patched += 1;
    }
    assert_eq!(
        patched, 2,
        "un en-tête local et un en-tête central attendus"
    );
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn encrypted_entries_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"secret", STORED)]);
    // Bit 0 du drapeau général : entrée chiffrée.
    patch_header_u16(&path, 6, 8, |flags| flags | 1);
    let mut archive = CbzArchive::open(&path).unwrap();
    assert!(archive.entries()[0].encrypted);
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::Encrypted)
    ));
}

#[test]
fn unsupported_compression_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"zstd?", STORED)]);
    // Méthode 93 : zstd, volontairement non compilé (bibliothèque C).
    patch_header_u16(&path, 8, 10, |_| 93);
    let mut archive = CbzArchive::open(&path).unwrap();
    let entry = &archive.entries()[0];
    assert!(!entry.compression_supported);
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::UnsupportedCompression(_))
    ));
}

#[test]
fn symlinks_are_identified() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.cbz");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    zip.add_symlink("lien.jpg", "a.jpg", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    let archive = CbzArchive::open(&path).unwrap();
    assert_eq!(archive.entries()[0].kind, EntryKind::Symlink);
}

#[test]
fn invalid_utf8_names_keep_their_raw_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("aé.jpg", b"x", STORED)]);
    // « é » en UTF-8 (c3 a9) remplacé par des octets invalides, drapeau UTF-8 conservé.
    replace_all(&path, "aé.jpg".as_bytes(), b"a\xff\xfe.jpg");
    let archive = CbzArchive::open(&path).unwrap();
    let entry = &archive.entries()[0];
    assert_eq!(entry.name_raw, b"a\xff\xfe.jpg");
    // `zip` retombe sur CP437 quand l'UTF-8 est invalide : le nom affiché est
    // approximatif, mais l'identité de la page repose sur les octets bruts.
    assert_eq!(entry.name, "a\u{a0}■.jpg");
}

#[test]
fn read_errors_are_classified() {
    use std::io::{Error, ErrorKind};
    assert!(matches!(
        ArchiveError::from_read_error(Error::new(ErrorKind::InvalidData, "Invalid checksum")),
        ArchiveError::CrcMismatch
    ));
    assert!(matches!(
        ArchiveError::from_read_error(Error::new(ErrorKind::InvalidData, "autre")),
        ArchiveError::Io(_)
    ));
    assert!(matches!(
        ArchiveError::from_read_error(Error::from(ErrorKind::UnexpectedEof)),
        ArchiveError::Truncated(_)
    ));
    assert!(matches!(
        ArchiveError::from_read_error(Error::from(ErrorKind::PermissionDenied)),
        ArchiveError::Io(_)
    ));
}

#[test]
fn zip_errors_are_classified() {
    use std::io::{Error, ErrorKind};
    type Expected = fn(&ArchiveError) -> bool;
    let cases: Vec<(ZipError, Expected)> = vec![
        (ZipError::Io(Error::from(ErrorKind::UnexpectedEof)), |e| {
            matches!(e, ArchiveError::Truncated(_))
        }),
        (ZipError::InvalidArchive("x".into()), |e| {
            matches!(e, ArchiveError::Unreadable(_))
        }),
        (ZipError::UnsupportedArchive("x"), |e| {
            matches!(e, ArchiveError::Unreadable(_))
        }),
        (
            ZipError::CompressionMethodNotSupported(93),
            |e| matches!(e, ArchiveError::UnsupportedCompression(m) if m.contains("93")),
        ),
        (ZipError::InvalidPassword, |e| {
            matches!(e, ArchiveError::Encrypted)
        }),
        (ZipError::FileNotFound, |e| {
            matches!(e, ArchiveError::Unreadable(_))
        }),
    ];
    for (zip_err, expected) in cases {
        let shown = zip_err.to_string();
        let err = ArchiveError::from(zip_err);
        assert!(expected(&err), "{shown} → {err:?}");
    }
}

#[test]
fn entries_larger_than_declared_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"abcdef", STORED)]);
    // Taille décompressée déclarée ramenée à 3 octets ; le CRC reste celui des 6 octets
    // réels, seule la limite de taille arrête la lecture.
    patch_header_u16(&path, 22, 24, |_| 3);
    let mut archive = CbzArchive::open(&path).unwrap();
    assert_eq!(archive.entries()[0].size, 3);
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::SizeMismatch)
    ));
}

#[test]
fn oversized_entries_are_refused_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"abcdef", STORED)]);
    // Mot de poids fort de la taille déclarée : 0x2000_0006 > 512 Mio.
    patch_header_u16(&path, 24, 26, |_| 0x2000);
    let mut archive = CbzArchive::open(&path).unwrap();
    assert!(archive.entries()[0].size > MAX_ENTRY_SIZE);
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::EntryTooLarge(_))
    ));
}
