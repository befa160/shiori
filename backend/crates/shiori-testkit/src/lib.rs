//! Fixtures de test : archives CBZ et CBR saines et cassées, générées par du code.
//!
//! Rien n'est versionné en binaire : les fixtures sont déterministes et identiques sous
//! Linux, Windows et macOS. Chaque page a une couleur unique ([`page_color`]) pour vérifier
//! qu'une page affichée est bien la bonne image.

use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use image::{ImageFormat, RgbImage};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

pub use rars::ArchiveVersion::{Rar29 as RAR4, Rar50 as RAR5};
pub use zip::CompressionMethod::{Deflated as DEFLATED, Stored as STORED};

/// Couleur unique d'une page, dérivée de son ordinal.
pub fn page_color(ordinal: usize) -> [u8; 3] {
    [(ordinal >> 8) as u8, ordinal as u8, 0x5a]
}

fn encode(width: u32, height: u32, rgb: [u8; 3], format: ImageFormat) -> Vec<u8> {
    let img = RgbImage::from_pixel(width, height, image::Rgb(rgb));
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

pub fn png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(width, height, rgb, ImageFormat::Png)
}

/// PNG de bruit déterministe, incompressible : son contenu occupe toute la place dans
/// l'archive, une corruption au milieu du fichier tombe dans des données de page.
pub fn noisy_png(width: u32, height: u32, seed: usize) -> Vec<u8> {
    let mut state = seed as u64 ^ 0x9e37_79b9_7f4a_7c15;
    let img = RgbImage::from_fn(width, height, |_, _| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let [r, g, b, ..] = state.to_le_bytes();
        image::Rgb([r, g, b])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

pub fn jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(width, height, rgb, ImageFormat::Jpeg)
}

pub fn webp(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    encode(width, height, rgb, ImageFormat::WebP)
}

/// GIF 1×1 minimal (format reconnu mais non pris en charge comme page).
pub const GIF_1X1: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

/// AVIF 8×12 rouge (200, 30, 30), 287 octets, encodé une fois avec `ravif` 0.13
/// (vitesse 10, qualité 50) : pas d'encodeur AV1 dans les dépendances de test.
pub const AVIF_8X12: &[u8] = &[
    0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x61, 0x76, 0x69, 0x66, 0x00, 0x00, 0x00, 0x00,
    0x6d, 0x69, 0x66, 0x31, 0x6d, 0x69, 0x61, 0x66, 0x00, 0x00, 0x00, 0xd2, 0x6d, 0x65, 0x74, 0x61,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x68, 0x64, 0x6c, 0x72, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x70, 0x69, 0x63, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x70, 0x69, 0x74, 0x6d, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1e, 0x69, 0x6c, 0x6f, 0x63, 0x00, 0x00, 0x00, 0x00, 0x44,
    0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xf2, 0x00, 0x00, 0x00,
    0x2d, 0x00, 0x00, 0x00, 0x23, 0x69, 0x69, 0x6e, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
    0x00, 0x00, 0x15, 0x69, 0x6e, 0x66, 0x65, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x61,
    0x76, 0x30, 0x31, 0x00, 0x00, 0x00, 0x00, 0x56, 0x69, 0x70, 0x72, 0x70, 0x00, 0x00, 0x00, 0x38,
    0x69, 0x70, 0x63, 0x6f, 0x00, 0x00, 0x00, 0x14, 0x69, 0x73, 0x70, 0x65, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x0c, 0x61, 0x76, 0x31, 0x43,
    0x81, 0x3f, 0x40, 0x00, 0x00, 0x00, 0x00, 0x10, 0x70, 0x69, 0x78, 0x69, 0x00, 0x00, 0x00, 0x00,
    0x03, 0x0a, 0x0a, 0x0a, 0x00, 0x00, 0x00, 0x16, 0x69, 0x70, 0x6d, 0x61, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x82, 0x03, 0x00, 0x00, 0x00, 0x35, 0x6d, 0x64,
    0x61, 0x74, 0x12, 0x00, 0x0a, 0x08, 0x3f, 0xc8, 0xfd, 0x91, 0x80, 0x86, 0x83, 0x68, 0x32, 0x1f,
    0x66, 0x26, 0x43, 0x43, 0x1d, 0x15, 0xf4, 0xc0, 0x00, 0x20, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x02, 0xb1, 0xee, 0xf5, 0xef, 0xb9, 0x9f, 0x1d, 0x5b, 0x6a, 0xc1,
];

/// Écrit une archive zip. Un nom finissant par `/` crée un dossier.
pub fn write_zip(dir: &Path, name: &str, files: &[(&str, &[u8], CompressionMethod)]) -> PathBuf {
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

/// Écrit une archive RAR, compressée ; `solid` : une seule suite compressée pour tout le
/// tome (pas d'accès direct à une page).
pub fn write_rar(
    dir: &Path,
    name: &str,
    files: &[(&str, &[u8])],
    version: rars::ArchiveVersion,
    solid: bool,
) -> PathBuf {
    let path = dir.join(name);
    let mut rar = rars::Builder::new(version).solid(solid);
    for (entry, content) in files {
        rar.add_bytes(entry.as_bytes().to_vec(), content.to_vec(), None, None)
            .unwrap();
    }
    rar.write_to_path(&path, None).unwrap();
    path
}

/// Remplace toutes les occurrences d'un motif de même longueur dans un fichier.
pub fn replace_all(path: &Path, from: &[u8], to: &[u8]) {
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

/// Applique `f` au champ u16 situé à `local_off` dans chaque en-tête local et à
/// `central_off` dans chaque en-tête de l'annuaire central.
pub fn patch_header_u16(path: &Path, local_off: usize, central_off: usize, f: impl Fn(u16) -> u16) {
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
    assert!(patched >= 2, "en-têtes introuvables");
    std::fs::write(path, bytes).unwrap();
}

/// Les fixtures nommées, une par situation du cahier des charges.
pub mod fixtures {
    use super::*;

    /// Nombre de pages du faux webtoon.
    pub const WEBTOON_PAGES: usize = 300;

    /// Faux webtoon : 300 pages PNG très hautes (32×2400), noms zéro-paddés.
    pub fn webtoon(dir: &Path) -> PathBuf {
        let pages: Vec<(String, Vec<u8>)> = (0..WEBTOON_PAGES)
            .map(|i| (format!("{:03}.png", i + 1), png(32, 2400, page_color(i))))
            .collect();
        let files: Vec<_> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
            .collect();
        write_zip(dir, "webtoon.cbz", &files)
    }

    /// Nombre de pages du faux manga (couverture et 4ème comprises).
    pub const MANGA_PAGES: usize = 12;

    /// Faux manga : couverture JPEG, pages simples, deux doubles pages (ratio 1,33),
    /// 4ème de couverture distincte, et un `ComicInfo.xml`.
    pub fn manga(dir: &Path) -> PathBuf {
        let mut pages = vec![(
            "000 - couverture.jpg".to_string(),
            jpeg(600, 900, page_color(0)),
        )];
        for i in 1..MANGA_PAGES - 1 {
            let (w, h) = if i == 4 || i == 7 {
                (1200, 900)
            } else {
                (600, 900)
            };
            pages.push((format!("{i:03}.png"), png(w, h, page_color(i))));
        }
        let last = MANGA_PAGES - 1;
        pages.push((
            format!("{last:03} - 4e.png"),
            png(600, 900, page_color(last)),
        ));
        let mut files: Vec<_> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), DEFLATED))
            .collect();
        files.push((
            "ComicInfo.xml",
            b"<ComicInfo><Title>Test</Title></ComicInfo>",
            DEFLATED,
        ));
        write_zip(dir, "manga.cbz", &files)
    }

    fn small(i: usize) -> Vec<u8> {
        png(8, 12, page_color(i))
    }

    /// Noms non paddés dans l'ordre de l'archive `1, 10, 2…` (l'ordre naturel diffère).
    pub fn unpadded(dir: &Path) -> PathBuf {
        let order = [1, 10, 11, 2, 3, 4, 5, 6, 7, 8, 9];
        let pages: Vec<_> = order
            .iter()
            .map(|&n| (format!("{n}.png"), small(n - 1)))
            .collect();
        let files: Vec<_> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
            .collect();
        write_zip(dir, "unpadded.cbz", &files)
    }

    /// Pages réparties dans deux sous-dossiers, plus fichiers parasites et entrées ignorées.
    pub fn messy(dir: &Path) -> PathBuf {
        let (a, b, c) = (small(0), small(1), small(2));
        write_zip(
            dir,
            "messy.cbz",
            &[
                ("ch1/", b"", STORED),
                ("ch1/01.png", &a, STORED),
                ("ch2/01.png", &b, STORED),
                ("ch2/02.png", &c, STORED),
                ("lisez-moi.txt", b"scan par X", DEFLATED),
                ("Thumbs.db", b"junk", STORED),
                ("__MACOSX/ch1/._01.png", b"junk", STORED),
            ],
        )
    }

    /// Deux entrées de même nom : contenu identique si `identical`, différent sinon.
    pub fn duplicates(dir: &Path, identical: bool) -> PathBuf {
        let (a, b) = (small(0), small(if identical { 0 } else { 1 }));
        let name = if identical {
            "dup-same.cbz"
        } else {
            "dup-conflict.cbz"
        };
        let path = write_zip(
            dir,
            name,
            &[
                ("01.png", &a, STORED),
                ("02.png", &small(2), STORED),
                ("0X.png", &b, STORED),
            ],
        );
        replace_all(&path, b"0X.png", b"01.png");
        path
    }

    /// Une page dont le contenu stocké est altéré après coup : CRC invalide.
    pub fn bad_crc(dir: &Path) -> PathBuf {
        let mut page = small(1);
        let marker = b"SHIORI-CORRUPT-ME";
        page.extend_from_slice(marker); // octets après IEND : l'image reste lisible
        let path = write_zip(
            dir,
            "bad-crc.cbz",
            &[("01.png", &small(0), STORED), ("02.png", &page, STORED)],
        );
        replace_all(&path, marker, b"SHIORI-CORRUPTED!");
        path
    }

    /// Archive coupée en son milieu : l'annuaire central a disparu.
    pub fn truncated(dir: &Path) -> PathBuf {
        let path = webtoon_like(dir, "truncated.cbz", 4);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        path
    }

    fn webtoon_like(dir: &Path, name: &str, pages: usize) -> PathBuf {
        let pages: Vec<_> = (0..pages)
            .map(|i| (format!("{:02}.png", i + 1), small(i)))
            .collect();
        let files: Vec<_> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
            .collect();
        write_zip(dir, name, &files)
    }

    /// Entrée marquée chiffrée (bit 0 du drapeau général).
    pub fn encrypted(dir: &Path) -> PathBuf {
        let path = webtoon_like(dir, "encrypted.cbz", 1);
        patch_header_u16(&path, 6, 8, |flags| flags | 1);
        path
    }

    /// `.jpg` qui contient du PNG, `.png` à l'en-tête cassé, GIF non pris en charge,
    /// `.webp` qui n'est pas une image.
    pub fn odd_images(dir: &Path) -> PathBuf {
        write_zip(
            dir,
            "odd-images.cbz",
            &[
                ("01.jpg", &small(0), STORED),
                ("02.png", b"\x89PNG pas vraiment", STORED),
                ("03.gif", GIF_1X1, STORED),
                ("04.png", &small(3), STORED),
                ("05.webp", b"du texte, pas une image", STORED),
            ],
        )
    }

    /// JPEG dont l'en-tête est intact mais le corps tronqué : visible seulement en `--deep`.
    pub fn corrupt_body(dir: &Path) -> PathBuf {
        let full = jpeg(64, 64, [200, 30, 30]);
        let cut = &full[..full.len() * 2 / 3];
        write_zip(
            dir,
            "corrupt-body.cbz",
            &[("01.jpg", cut, STORED), ("02.png", &small(1), STORED)],
        )
    }

    /// Une page de chaque format pris en charge : jpg, png, webp, avif.
    pub fn all_formats(dir: &Path) -> PathBuf {
        let (j, p, w) = (
            jpeg(8, 12, page_color(0)),
            png(8, 12, page_color(1)),
            webp(8, 12, page_color(2)),
        );
        write_zip(
            dir,
            "all-formats.cbz",
            &[
                ("1.jpg", &j, STORED),
                ("2.png", &p, STORED),
                ("3.webp", &w, STORED),
                ("4.avif", AVIF_8X12, STORED),
            ],
        )
    }

    /// Archive sans aucune image.
    pub fn no_images(dir: &Path) -> PathBuf {
        write_zip(
            dir,
            "no-images.cbz",
            &[("notes.txt", b"rien ici", DEFLATED)],
        )
    }

    /// Fichier `.cbz` qui n'est pas un zip.
    /// RAR solid de 8 pages de bruit, abîmé au milieu de ses données compressées : le
    /// décodage échoue à partir d'une page et toutes les suivantes en dépendent.
    pub fn damaged_solid_rar(dir: &Path) -> (PathBuf, Vec<Vec<u8>>) {
        let pages: Vec<Vec<u8>> = (0..8).map(|i| noisy_png(64, 64, i)).collect();
        let names: Vec<String> = (0..8).map(|i| format!("{i}.png")).collect();
        let files: Vec<(&str, &[u8])> = names
            .iter()
            .zip(&pages)
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        let path = write_rar(dir, "abime.cbr", &files, RAR5, true);
        let mut bytes = std::fs::read(&path).unwrap();
        let middle = bytes.len() / 2;
        for b in &mut bytes[middle..middle + 16] {
            *b ^= 0xff;
        }
        std::fs::write(&path, bytes).unwrap();
        (path, pages)
    }

    /// Nombre de pages des faux mangas RAR.
    pub const RAR_PAGES: usize = 6;

    /// Faux manga RAR : deux chapitres en sous-dossiers, pages écrites dans l'ordre inverse
    /// de lecture, et un `ComicInfo.xml`. La page d'ordinal `i` a la couleur `page_color(i)`.
    pub fn rar(dir: &Path, name: &str, version: rars::ArchiveVersion, solid: bool) -> PathBuf {
        let pages: Vec<(String, Vec<u8>)> = (0..RAR_PAGES)
            .rev()
            .map(|i| {
                let name = format!("ch{}/{:02}.png", i / 3 + 1, i % 3 + 1);
                (name, png(60, 90, page_color(i)))
            })
            .collect();
        let mut files: Vec<(&str, &[u8])> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        files.push(("ComicInfo.xml", b"<ComicInfo/>"));
        write_rar(dir, name, &files, version, solid)
    }

    pub fn not_a_zip(dir: &Path) -> PathBuf {
        let path = dir.join("not-a-zip.cbz");
        std::fs::write(&path, b"Ceci n'est pas une archive.").unwrap();
        path
    }
}
