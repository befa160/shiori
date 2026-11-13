//! Bibliothèque des tests de bout en bout du lecteur (`web/e2e`) : un manga CBZ (couverture
//! JPEG, deux doubles pages), un manga RAR solid écrit à l'envers de l'ordre de lecture et un
//! webtoon. Écrit aussi `manifest.json` : pour chaque fichier, la couleur attendue de chaque
//! page dans l'ordre de lecture. Deux pages voisines ont des couleurs très éloignées : afficher
//! la page d'à côté ne passe jamais inaperçu, même après un encodage AVIF avec pertes.
//!
//! `cargo run -p shiori-testkit --example e2e_library -- <dossier>`

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use shiori_testkit::{DEFLATED, RAR5, STORED, jpeg, png, write_rar, write_zip};

/// Teinte tournant de l'angle d'or à chaque page, saturée et claire.
fn color(ordinal: usize) -> [u8; 3] {
    let hue = (ordinal as f64 * 137.508) % 360.0;
    let (s, v) = (0.85, 0.9);
    let c = v * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|u| ((u + m) * 255.0).round() as u8)
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("usage : e2e_library <dossier>");
    let root = Path::new(&root);
    let mut manifest = Vec::new();

    // Manga CBZ : couverture, pages 600×900, doubles pages 1200×900 aux pages 5 et 8.
    let manga = root.join("manga/E2E Manga");
    fs::create_dir_all(&manga).unwrap();
    let pages: Vec<(String, Vec<u8>)> = (0..12)
        .map(|i| match i {
            0 => ("000 - couverture.jpg".into(), jpeg(600, 900, color(i))),
            4 | 7 => (format!("{i:03}.png"), png(1200, 900, color(i))),
            _ => (format!("{i:03}.png"), png(600, 900, color(i))),
        })
        .collect();
    let files: Vec<_> = pages
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), DEFLATED))
        .collect();
    write_zip(&manga, "E2E Manga T01.cbz", &files);
    manifest.push(("E2E Manga T01.cbz", 12));

    // Manga RAR5 solid : deux chapitres, pages écrites dans l'ordre inverse de lecture.
    let pages: Vec<(String, Vec<u8>)> = (0..6)
        .rev()
        .map(|i| {
            (
                format!("ch{}/{:02}.png", i / 3 + 1, i % 3 + 1),
                png(600, 900, color(i)),
            )
        })
        .collect();
    let files: Vec<(&str, &[u8])> = pages
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    write_rar(&manga, "E2E Manga T02.cbr", &files, RAR5, true);
    manifest.push(("E2E Manga T02.cbr", 6));

    // Webtoon : 60 bandes de 400×1200, servies en originales (PNG exact).
    let strip = root.join("webtoon/E2E Strip");
    fs::create_dir_all(&strip).unwrap();
    let pages: Vec<(String, Vec<u8>)> = (0..60)
        .map(|i| (format!("{:03}.png", i + 1), png(400, 1200, color(i))))
        .collect();
    let files: Vec<_> = pages
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
        .collect();
    write_zip(&strip, "E2E Strip T01.cbz", &files);
    manifest.push(("E2E Strip T01.cbz", 60));

    // Métadonnées (phase 5) : un tome avec ComicInfo.xml, hors des bibliothèques du setup ; le
    // parcours `metadata.spec.ts` en fait sa propre bibliothèque. Titre de série différent du
    // dossier, pour voir qu'il l'emporte.
    let meta = root.join("meta/E2E Meta");
    fs::create_dir_all(&meta).unwrap();
    let info = "<ComicInfo><Series>Série Méta E2E</Series><Writer>Autrice E2E</Writer>\
        <Summary>Résumé venu du fichier.</Summary><Genre>Aventure</Genre></ComicInfo>";
    let cover = png(600, 900, color(0));
    let files = [
        ("001.png", cover.as_slice(), DEFLATED),
        ("ComicInfo.xml", info.as_bytes(), DEFLATED),
    ];
    write_zip(&meta, "E2E Meta T01.cbz", &files);

    let mut json = String::from("{");
    for (k, (name, count)) in manifest.iter().enumerate() {
        let colors: Vec<String> = (0..*count).map(|i| format!("{:?}", color(i))).collect();
        let sep = if k == 0 { "" } else { "," };
        write!(json, "{sep}\n  \"{name}\": [{}]", colors.join(", ")).unwrap();
    }
    json.push_str("\n}\n");
    fs::write(root.join("manifest.json"), json).unwrap();
}
