use std::io::Write as _;

use shiori_testkit::pdf::{
    PAGE_HEIGHT, PAGE_WIDTH, PdfImage, PdfPage, Space, encrypted_pdf, write_pdf,
};
use shiori_testkit::{STORED, jpeg, page_color, write_zip};

use super::*;

fn codes(archive: &PdfArchive) -> Vec<(IssueCode, Option<&str>)> {
    archive
        .issues()
        .iter()
        .map(|i| (i.code, i.entry.as_deref()))
        .collect()
}

fn decoded(bytes: &[u8]) -> DynamicImage {
    image::load_from_memory(bytes).unwrap()
}

#[test]
fn each_jpeg_page_is_served_as_is() {
    let dir = tempfile::tempdir().unwrap();
    let pages: Vec<_> = (0..3)
        .map(|i| PdfPage::image(PdfImage::full(30, 40, page_color(i), true)))
        .collect();
    let path = write_pdf(dir.path(), "comic.pdf", &pages);
    assert!(is_pdf(&path));

    let mut archive = PdfArchive::open(&path).unwrap();
    let names: Vec<_> = archive.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["0001.jpg", "0002.jpg", "0003.jpg"]);
    assert!(archive.issues().is_empty());
    assert!(archive.duplicates().unwrap().is_empty());
    for i in 0..3 {
        // L'image incluse, à l'octet près : ni marge ni réencodage.
        assert_eq!(archive.read_entry(i).unwrap(), jpeg(30, 40, page_color(i)));
    }
    assert!(matches!(
        archive.read_entry(3),
        Err(ArchiveError::NoSuchEntry(3))
    ));
}

#[test]
fn pages_without_a_large_enough_image_are_skipped_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let full = PdfImage::full(30, 40, page_color(0), true);
    let quarter = PdfImage {
        at: [0.0, 0.0, PAGE_WIDTH / 2.0, PAGE_HEIGHT / 2.0],
        ..full.clone()
    };
    let pages = [
        PdfPage::image(full.clone()),
        PdfPage {
            text: Some(true),
            ..PdfPage::default()
        },
        PdfPage::image(quarter),
        PdfPage {
            text: Some(true),
            ..PdfPage::image(full.clone())
        },
        PdfPage {
            text: Some(false),
            ..PdfPage::image(full)
        },
    ];
    let path = write_pdf(dir.path(), "mixed.pdf", &pages);
    let archive = PdfArchive::open(&path).unwrap();
    let names: Vec<_> = archive.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["0001.jpg", "0004.jpg", "0005.jpg"]);
    // La couche OCR invisible de la page 5 n'est pas signalée.
    assert_eq!(
        codes(&archive),
        [
            (IssueCode::PdfPageWithoutImage, Some("page 2")),
            (IssueCode::PdfPageWithoutImage, Some("page 3")),
            (IssueCode::PdfTextIgnored, Some("page 4")),
        ]
    );
}

#[test]
fn other_images_are_decoded_to_png() {
    let dir = tempfile::tempdir().unwrap();
    let raw = PdfImage::full(30, 40, [200, 30, 30], false);
    let turned = PdfImage::full(30, 40, [10, 20, 30], true);
    let pages = [
        PdfPage::image(raw),
        PdfPage {
            rotate: 90,
            ..PdfPage::image(turned)
        },
    ];
    let path = write_pdf(dir.path(), "decoded.pdf", &pages);
    let mut archive = PdfArchive::open(&path).unwrap();
    let names: Vec<_> = archive.entries().iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["0001.png", "0002.png"]);

    let first = decoded(&archive.read_entry(0).unwrap()).to_rgb8();
    assert_eq!(first.dimensions(), (30, 40));
    assert_eq!(first.get_pixel(5, 5).0, [200, 30, 30]);
    // Page tournée d'un quart de tour : l'image est servie tournée.
    let second = decoded(&archive.read_entry(1).unwrap());
    assert_eq!((second.width(), second.height()), (40, 30));
}

#[test]
fn a_one_bit_mask_is_black_on_white_and_icc_jpeg_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mask = PdfImage {
        space: Space::Mask,
        ..PdfImage::full(16, 8, [0, 0, 0], false)
    };
    let icc = PdfImage {
        space: Space::IccRgb,
        ..PdfImage::full(30, 40, page_color(1), true)
    };
    let path = write_pdf(
        dir.path(),
        "mask.pdf",
        &[PdfPage::image(mask), PdfPage::image(icc)],
    );
    let mut archive = PdfArchive::open(&path).unwrap();
    let names: Vec<_> = archive.entries().iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["0001.png", "0002.jpg"]);
    let page = decoded(&archive.read_entry(0).unwrap()).to_luma8();
    assert_eq!(page.dimensions(), (16, 8));
    // Pixel peint : noir, pas en négatif.
    assert_eq!(page.get_pixel(3, 3).0, [0]);
    assert_eq!(archive.read_entry(1).unwrap(), jpeg(30, 40, page_color(1)));
}

#[test]
fn an_image_announcing_too_many_pixels_is_not_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let trap = PdfImage {
        declared: Some((100_000, 100_000)),
        ..PdfImage::full(4, 4, [9, 9, 9], false)
    };
    let path = write_pdf(dir.path(), "trap.pdf", &[PdfPage::image(trap)]);
    let mut archive = PdfArchive::open(&path).unwrap();
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::ImageUndecodable(m)) if m.contains("trop grande")
    ));
}

#[test]
fn read_each_gives_every_page_once_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let pages: Vec<_> = (0..4)
        .map(|i| PdfPage::image(PdfImage::full(30, 40, page_color(i), i % 2 == 0)))
        .collect();
    let path = write_pdf(dir.path(), "each.pdf", &pages);
    let mut archive = PdfArchive::open(&path).unwrap();
    let mut seen = Vec::new();
    archive.read_each(&[3, 1, 0, 2, 1, 9], &mut |i, read| {
        seen.push((i, read.map(|b| decoded(&b).to_rgb8().get_pixel(0, 0).0)));
    });
    let ok: Vec<_> = seen[..4].iter().map(|(i, r)| (*i, r.is_ok())).collect();
    assert_eq!(ok, [(0, true), (1, true), (2, true), (3, true)]);
    assert_eq!(*seen[1].1.as_ref().unwrap(), page_color(1));
    assert!(matches!(seen[4], (9, Err(ArchiveError::NoSuchEntry(9)))));
}

#[test]
fn a_page_of_strips_is_composed_without_its_margins() {
    let dir = tempfile::tempdir().unwrap();
    // Deux bandes de 30 × 20 pixels l'une sur l'autre, dans des marges de 60 et 40 points.
    let strip = |y: f32, rgb| PdfImage {
        at: [60.0, y, 480.0, 360.0],
        ..PdfImage::full(30, 20, rgb, false)
    };
    let page = PdfPage {
        images: vec![strip(40.0, [200, 0, 0]), strip(400.0, [0, 0, 200])],
        ..PdfPage::default()
    };
    let path = write_pdf(dir.path(), "strips.pdf", &[page]);
    let mut archive = PdfArchive::open(&path).unwrap();
    assert_eq!(archive.entries()[0].name, "0001.png");
    let img = decoded(&archive.read_entry(0).unwrap()).to_rgb8();
    assert_eq!(img.dimensions(), (30, 40));
    // La bande dessinée plus haut sur la page (y PDF plus grand) est en haut de l'image.
    assert_eq!(img.get_pixel(15, 5).0, [0, 0, 200]);
    assert_eq!(img.get_pixel(15, 35).0, [200, 0, 0]);
}

#[test]
fn tiles_take_the_finest_density_and_layers_keep_their_transparency() {
    let dir = tempfile::tempdir().unwrap();
    let left = PdfImage {
        at: [0.0, 0.0, PAGE_WIDTH / 2.0, PAGE_HEIGHT],
        ..PdfImage::full(10, 40, [200, 0, 0], false)
    };
    let right = PdfImage {
        at: [PAGE_WIDTH / 2.0, 0.0, PAGE_WIDTH / 2.0, PAGE_HEIGHT],
        ..PdfImage::full(30, 40, [0, 200, 0], false)
    };
    // Calque transparent par-dessus le fond : le fond reste visible, pas du blanc.
    let back = PdfImage::full(20, 20, [0, 0, 200], false);
    let clear = PdfImage {
        alpha: Some(0),
        ..PdfImage::full(20, 20, [9, 9, 9], false)
    };
    let pages = [
        PdfPage {
            images: vec![left, right],
            ..PdfPage::default()
        },
        PdfPage {
            images: vec![back, clear],
            ..PdfPage::default()
        },
    ];
    let path = write_pdf(dir.path(), "tiles.pdf", &pages);
    let mut archive = PdfArchive::open(&path).unwrap();

    let tiles = decoded(&archive.read_entry(0).unwrap()).to_rgb8();
    // 30 pixels pour 300 points à droite : la gauche (10 pixels) est agrandie à 30.
    assert_eq!(tiles.dimensions(), (60, 40));
    assert_eq!(tiles.get_pixel(5, 20).0, [200, 0, 0]);
    assert_eq!(tiles.get_pixel(55, 20).0, [0, 200, 0]);

    let layers = decoded(&archive.read_entry(1).unwrap()).to_rgb8();
    assert_eq!(layers.get_pixel(10, 10).0, [0, 0, 200]);
}

#[test]
fn a_tiny_piece_does_not_blow_up_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let speck = PdfImage {
        at: [10.0, 10.0, 0.01, 0.01],
        ..PdfImage::full(1, 1, [0, 0, 0], false)
    };
    let page = PdfPage {
        images: vec![PdfImage::full(30, 40, [200, 0, 0], false), speck],
        ..PdfPage::default()
    };
    let path = write_pdf(dir.path(), "speck.pdf", &[page]);
    let mut archive = PdfArchive::open(&path).unwrap();
    let img = decoded(&archive.read_entry(0).unwrap()).to_rgb8();
    assert_eq!(img.dimensions(), (30, 40));
}

#[test]
fn a_changed_image_is_a_crc_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let page = PdfPage::image(PdfImage::full(30, 40, page_color(0), true));
    let path = write_pdf(dir.path(), "changed.pdf", &[page]);
    let mut archive = PdfArchive::open(&path).unwrap();
    let range = archive.sources[0].jpeg.clone().unwrap();

    let mut bytes = std::fs::read(&path).unwrap();
    bytes[range.end - 3] ^= 0xff;
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    assert!(matches!(
        archive.read_entry(0),
        Err(ArchiveError::CrcMismatch)
    ));
}

#[test]
fn encrypted_and_broken_files_fail_to_open() {
    let dir = tempfile::tempdir().unwrap();
    let encrypted = encrypted_pdf(dir.path());
    assert!(matches!(
        PdfArchive::open(&encrypted),
        Err(ArchiveError::Encrypted)
    ));

    let junk = dir.path().join("junk.pdf");
    std::fs::write(&junk, b"%PDF-1.7\nnothing here\n%%EOF\n").unwrap();
    assert!(is_pdf(&junk));
    assert!(matches!(
        PdfArchive::open(&junk),
        Err(ArchiveError::Unreadable(_))
    ));

    // Copie interrompue : hayro rouvrirait le début, mais la fin manque.
    let page = PdfPage::image(PdfImage::full(30, 40, page_color(0), true));
    let whole = std::fs::read(write_pdf(dir.path(), "whole.pdf", &[page.clone(), page])).unwrap();
    let cut = dir.path().join("cut.pdf");
    std::fs::write(&cut, &whole[..whole.len() * 2 / 3]).unwrap();
    assert!(matches!(
        PdfArchive::open(&cut),
        Err(ArchiveError::Truncated(_))
    ));

    // Un CBZ dont la première entrée est un PDF stocké tel quel reste un CBZ.
    let cbz = write_zip(dir.path(), "inner.cbz", &[("a.pdf", &whole, STORED)]);
    assert!(!is_pdf(&cbz));
    assert!(matches!(
        PdfArchive::open(&dir.path().join("missing.pdf")),
        Err(ArchiveError::Io(_))
    ));
    assert!(!is_pdf(&dir.path().join("missing.pdf")));
}

#[test]
fn orientation_follows_the_drawing_matrix() {
    use Orientation::*;
    let cases = [
        ([1.0, 0.0, 0.0, 1.0], Upright),
        ([-1.0, 0.0, 0.0, 1.0], FlipH),
        ([1.0, 0.0, 0.0, -1.0], FlipV),
        ([-1.0, 0.0, 0.0, -1.0], Rotate180),
        ([0.0, 1.0, -1.0, 0.0], Rotate90),
        ([0.0, -1.0, 1.0, 0.0], Rotate270),
        ([0.0, 1.0, 1.0, 0.0], Transpose),
        ([0.0, -1.0, -1.0, 0.0], Transverse),
    ];
    // Une image 2 × 1 : pixel gauche noir, droit blanc.
    let img = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![0, 255]).unwrap()).to_rgba8();
    for ([a, b, c, d], want) in cases {
        let m = Affine::new([a, b, c, d, 0.0, 0.0]);
        assert_eq!(orientation(m), want);
        let out = DynamicImage::ImageRgba8(orient(img.clone(), m)).to_luma8();
        // Le pixel noir (0, 0) de l'image arrive là où la matrice l'envoie.
        let (w, h) = out.dimensions();
        let black = out
            .enumerate_pixels()
            .find(|(_, _, p)| p.0[0] == 0)
            .map(|(x, y, _)| (x, y))
            .unwrap();
        let corner = |v: f64, size: u32| if v < 0.0 { size - 1 } else { 0 };
        let want_at = (corner(a + c, w), corner(b + d, h));
        assert_eq!(black, want_at, "{want:?}");
    }
}

#[test]
fn transparency_is_kept_then_flattened_on_white() {
    let luma = |data: Vec<u8>| LumaData {
        data,
        width: 2,
        height: 1,
        interpolate: false,
        scale_factors: (1.0, 1.0),
    };
    let rgb = ImageData::Rgb(hayro_interpret::RgbData {
        data: vec![0, 0, 0, 0, 0, 0],
        width: 2,
        height: 1,
        interpolate: false,
        scale_factors: (1.0, 1.0),
    });
    let img = to_image(rgb, Some(luma(vec![255, 0]))).unwrap();
    assert_eq!(img.as_raw(), &[0, 0, 0, 255, 0, 0, 0, 0]);
    let page = compose(vec![(img, Rect::new(0.0, 0.0, 1.0, 1.0))]).unwrap();
    assert_eq!(page.as_raw(), &[0, 0, 0, 255, 255, 255, 255, 255]);

    // Un masque d'une autre taille est ignoré : l'image reste opaque.
    let other = LumaData {
        width: 1,
        ..luma(vec![0])
    };
    let kept = to_image(ImageData::Luma(luma(vec![7, 7])), Some(other)).unwrap();
    assert_eq!(kept.as_raw(), &[7, 7, 7, 255, 7, 7, 7, 255]);

    assert!(compose(Vec::new()).is_err());
    // Deux images qui demanderaient des Go de pixels une fois recomposées.
    let dot = || RgbaImage::new(1, 1);
    let huge = compose(vec![
        (dot(), Rect::new(0.0, 0.0, 0.001, 0.001)),
        (dot(), Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    ]);
    assert!(huge.unwrap_err().contains("trop grande"));
}
