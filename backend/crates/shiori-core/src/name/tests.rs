use super::*;

use NameKind::{Chapter, Special, Unknown, Volume};

/// (chemin relatif, série, type, numéro, fin d'intervalle)
type Case = (
    &'static str,
    &'static str,
    NameKind,
    Option<f64>,
    Option<f64>,
);

const CASES: &[Case] = &[
    // Dossier par série : la série est le dossier, le numéro vient du nom.
    (
        "One Piece/One Piece T01.cbz",
        "One Piece",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "One Piece/One Piece - Tome 105.cbz",
        "One Piece",
        Volume,
        Some(105.0),
        None,
    ),
    ("One Piece/Tome 1.cbz", "One Piece", Volume, Some(1.0), None),
    (
        "Naruto/Naruto Vol. 12.cbz",
        "Naruto",
        Volume,
        Some(12.0),
        None,
    ),
    (
        "Naruto/Naruto Vol.12.cbr",
        "Naruto",
        Volume,
        Some(12.0),
        None,
    ),
    ("Naruto/Naruto v12.cbz", "Naruto", Volume, Some(12.0), None),
    (
        "Naruto/Naruto Volume 012.cbz",
        "Naruto",
        Volume,
        Some(12.0),
        None,
    ),
    ("Naruto/naruto_v012.cbz", "Naruto", Volume, Some(12.0), None),
    (
        "Naruto/NARUTO TOME 3.CBZ",
        "Naruto",
        Volume,
        Some(3.0),
        None,
    ),
    ("Blame/Blame 03.cbz", "Blame", Volume, Some(3.0), None),
    ("Blame/03.cbz", "Blame", Volume, Some(3.0), None),
    ("Blame/03 - Le puits.cbz", "Blame", Volume, Some(3.0), None),
    ("Blame/Blame-03.cbz", "Blame", Volume, Some(3.0), None),
    (
        "Toriko/Toriko T16 (Mitsutoshi SHIMABUKURO) [eBook officiel 1200].cbr",
        "Toriko",
        Volume,
        Some(16.0),
        None,
    ),
    ("Toriko\\Toriko T16.cbr", "Toriko", Volume, Some(16.0), None),
    (
        "Leviathan/Leviathan v01.cbz",
        "Leviathan",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "Série/Sous-dossier/Tome 2.cbz",
        "Série",
        Volume,
        Some(2.0),
        None,
    ),
    ("  Spaced  /Tome 2.cbz", "Spaced", Volume, Some(2.0), None),
    // Noms réels d'une bibliothèque de téléchargements : dossiers bruités, série tirée du fichier.
    (
        "Vinland.Saga.INTEGRALE.Yukimura.FR.[CBZ]-thor1295\\Vinland.Saga.T05.Yukimura.FR.[CBZ]-thor1295.cbz",
        "Vinland Saga",
        Volume,
        Some(5.0),
        None,
    ),
    (
        "Birdmen (01-16) (Tanabe) (2020) [Digital-2100] (TONER-PapriKa+)\\Birdmen T10 (Tanabe) (2022) [Digital-2100] (TONER-PapriKa+).cbz",
        "Birdmen",
        Volume,
        Some(10.0),
        None,
    ),
    (
        "07-Ghost INTEGRAL [DIGITAL 1920] FR CBZ\\07-Ghost T16 [DIGITAL 1920] FR  NOTAG.cbz",
        "07-Ghost",
        Volume,
        Some(16.0),
        None,
    ),
    (
        "All.Out.Shiori.Amase.[INTEGRALE].FR.[CBZ]-Dogbanov\\All.Out.T08.Shiori.Amase.FR.[CBZ]-Dogbanov.cbz",
        "All Out",
        Volume,
        Some(8.0),
        None,
    ),
    (
        "LINK.Shono.World's.End.Harem.T01-T18.2018.French.[CBZ.PDF]-Booker\\CBZ\\World's End Harem - T03.cbz",
        "World's End Harem",
        Volume,
        Some(3.0),
        None,
    ),
    (
        "Battle Club 01-06\\Battle Club - 02.cbr",
        "Battle Club",
        Volume,
        Some(2.0),
        None,
    ),
    (
        "Gigant - 01-10\\Gigant 09.cbz",
        "Gigant",
        Volume,
        Some(9.0),
        None,
    ),
    (
        "Blazer Drive - 01-09\\Blazer Drive - T03.cbz",
        "Blazer Drive",
        Volume,
        Some(3.0),
        None,
    ),
    (
        "Noritaka\\Volume 11 VF_00067d.cbz",
        "Noritaka",
        Volume,
        Some(11.0),
        None,
    ),
    (
        "Pick Me Up !\\Chapitre 167_455d77.cbz",
        "Pick Me Up !",
        Chapter,
        Some(167.0),
        None,
    ),
    ("Gigant - 01-10\\09.cbz", "Gigant", Volume, Some(9.0), None),
    (
        "Battle Club 2nd Stage - 01-03\\01.cbz",
        "Battle Club 2nd Stage",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "Berserk (01-41+) (Miura) (2004) [Digital-1920]\\Tome 1.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "Hunter.X.Hunter.(T39).(TOGASHI).(2026).[Digital-1350].(Officiel).[CBZ].FR-NoFace696.cbz",
        "Hunter X Hunter",
        Volume,
        Some(39.0),
        None,
    ),
    (
        "My_Girlfriend_Is_a_Zombie_Intégrale[R3dHTam].cbz",
        "My Girlfriend Is a Zombie",
        Special,
        None,
        None,
    ),
    (
        "Leviathan (PalaceMultiUnivers).cbz",
        "Leviathan",
        Unknown,
        None,
        None,
    ),
    ("Akira (Tome 3).cbz", "Akira", Volume, Some(3.0), None),
    ("Akira (v2) (Tome).cbz", "Akira", Volume, Some(2.0), None),
    // Décimaux et intervalles.
    (
        "Berserk/Berserk Tome 1.5.cbz",
        "Berserk",
        Volume,
        Some(1.5),
        None,
    ),
    (
        "Berserk/Berserk Tome 1,5.cbz",
        "Berserk",
        Volume,
        Some(1.5),
        None,
    ),
    (
        "Berserk/Berserk v10.5.cbz",
        "Berserk",
        Volume,
        Some(10.5),
        None,
    ),
    (
        "Berserk/Berserk Tomes 1-3.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk T01-T03.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk T01 à 03.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk v01 to v03.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk v01~03.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk 01-03.cbz",
        "Berserk",
        Volume,
        Some(1.0),
        Some(3.0),
    ),
    (
        "Berserk/Berserk T03-T01.cbz",
        "Berserk",
        Volume,
        Some(3.0),
        None,
    ),
    (
        "Berserk/Berserk T03-03.cbz",
        "Berserk",
        Volume,
        Some(3.0),
        None,
    ),
    // Chapitres.
    (
        "Solo Leveling/Chapitre 12.cbz",
        "Solo Leveling",
        Chapter,
        Some(12.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling Chapter 110.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling Ch. 110.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling ch110.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling c110.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling Chap 110.5.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.5),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling #110.cbz",
        "Solo Leveling",
        Chapter,
        Some(110.0),
        None,
    ),
    (
        "Solo Leveling/Solo Leveling c100-105.cbz",
        "Solo Leveling",
        Chapter,
        Some(100.0),
        Some(105.0),
    ),
    (
        "Batman/Batman #12 (2016).cbz",
        "Batman",
        Chapter,
        Some(12.0),
        None,
    ),
    // Le tome l'emporte quand le nom porte tome et chapitre.
    (
        "One Piece/One Piece v01 c001.cbz",
        "One Piece",
        Volume,
        Some(1.0),
        None,
    ),
    // Japonais.
    (
        "ワンピース/ワンピース 第1巻.cbz",
        "ワンピース",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "ワンピース/第 12 巻.cbz",
        "ワンピース",
        Volume,
        Some(12.0),
        None,
    ),
    (
        "ワンピース/ワンピース 3巻.cbz",
        "ワンピース",
        Volume,
        Some(3.0),
        None,
    ),
    (
        "ワンピース/ワンピース 第105話.cbz",
        "ワンピース",
        Chapter,
        Some(105.0),
        None,
    ),
    // Spéciaux.
    (
        "One Piece/One Piece Hors-série 1.cbz",
        "One Piece",
        Special,
        Some(1.0),
        None,
    ),
    (
        "One Piece/One Piece Hors série.cbz",
        "One Piece",
        Special,
        None,
        None,
    ),
    (
        "One Piece/One Piece HS 2.cbz",
        "One Piece",
        Special,
        Some(2.0),
        None,
    ),
    (
        "One Piece/One Piece - Special.cbz",
        "One Piece",
        Special,
        None,
        None,
    ),
    (
        "One Piece/One Piece Spécial 3.cbz",
        "One Piece",
        Special,
        Some(3.0),
        None,
    ),
    (
        "One Piece/One Piece Extra 2.cbz",
        "One Piece",
        Special,
        Some(2.0),
        None,
    ),
    (
        "One Piece/One Piece Extras.cbz",
        "One Piece",
        Special,
        None,
        None,
    ),
    (
        "One Piece/One Piece Omake.cbz",
        "One Piece",
        Special,
        None,
        None,
    ),
    ("Akira/Akira One-shot.cbz", "Akira", Special, None, None),
    ("Akira/Akira oneshot.cbz", "Akira", Special, None, None),
    (
        "Akira/Akira Intégrale 2.cbz",
        "Akira",
        Special,
        Some(2.0),
        None,
    ),
    ("Akira/Akira Integrale.cbz", "Akira", Special, None, None),
    (
        "Akira/Akira Intégrale #1-2.cbz",
        "Akira",
        Special,
        Some(1.0),
        Some(2.0),
    ),
    // Un mot « spécial » dans le titre de la série ne fait pas un spécial.
    (
        "Extra Life/Extra Life T01.cbz",
        "Extra Life",
        Volume,
        Some(1.0),
        None,
    ),
    (
        "Special A/Special A T03.cbz",
        "Special A",
        Volume,
        Some(3.0),
        None,
    ),
    ("Extra Life T02.cbz", "Extra Life", Volume, Some(2.0), None),
    (
        "Special A/Special A Extra 2.cbz",
        "Special A",
        Special,
        Some(2.0),
        None,
    ),
    // Tags ignorés, années jamais prises pour un numéro.
    (
        "Akira/Akira [Glénat] T02 (Digital).cbz",
        "Akira",
        Volume,
        Some(2.0),
        None,
    ),
    (
        "Akira/Akira {Team} v02.cbz",
        "Akira",
        Volume,
        Some(2.0),
        None,
    ),
    (
        "Akira/Akira (2019) 02.cbz",
        "Akira",
        Volume,
        Some(2.0),
        None,
    ),
    ("Akira/Akira 1988.cbz", "Akira", Unknown, None, None),
    ("Akira/2019.cbz", "Akira", Unknown, None, None),
    (
        "Akira/Akira 1988 - 2019.cbz",
        "Akira",
        Volume,
        Some(1988.0),
        Some(2019.0),
    ),
    ("Akira/Akira T2019.cbz", "Akira", Volume, Some(2019.0), None),
    ("Akira/Akira 3000.cbz", "Akira", Volume, Some(3000.0), None),
    ("Akira/Akira 1850.cbz", "Akira", Volume, Some(1850.0), None),
    ("Akira/Akira 201.cbz", "Akira", Volume, Some(201.0), None),
    // Pas de numéro : type inconnu.
    ("Akira/Akira.cbz", "Akira", Unknown, None, None),
    ("Akira/Le grand livre.cbz", "Akira", Unknown, None, None),
    ("Akira/Part Two.cbz", "Akira", Unknown, None, None),
    ("Akira/Ghost in the Shell.cbz", "Akira", Unknown, None, None),
    (
        "20th Century Boys/20th Century Boys.cbz",
        "20th Century Boys",
        Unknown,
        None,
        None,
    ),
    (
        "20th Century Boys/20th Century Boys 05.cbz",
        "20th Century Boys",
        Volume,
        Some(5.0),
        None,
    ),
    (
        "3 Gatsu no Lion/3 Gatsu no Lion 05.cbz",
        "3 Gatsu no Lion",
        Volume,
        Some(5.0),
        None,
    ),
    // Fichiers posés à la racine : la série vient du nom.
    ("One Piece T01.cbz", "One Piece", Volume, Some(1.0), None),
    ("One.Piece.v01.cbz", "One Piece", Volume, Some(1.0), None),
    (
        "One_Piece_-_Tome_01.cbz",
        "One Piece",
        Volume,
        Some(1.0),
        None,
    ),
    ("Blame 03.cbz", "Blame", Volume, Some(3.0), None),
    ("Blame 1.5.cbz", "Blame", Volume, Some(1.5), None),
    (
        "Solo Leveling Chapitre 12.cbz",
        "Solo Leveling",
        Chapter,
        Some(12.0),
        None,
    ),
    ("Akira Intégrale.cbz", "Akira", Special, None, None),
    ("Akira.cbz", "Akira", Unknown, None, None),
    ("Akira [Glénat].cbz", "Akira", Unknown, None, None),
    ("T01.cbz", "T01", Volume, Some(1.0), None),
    ("03.cbz", "03", Volume, Some(3.0), None),
    ("Tome 1.5", "Tome 1.5", Volume, Some(1.5), None),
    ("archive.tar.cbz", "archive tar", Unknown, None, None),
    (".cbz", "cbz", Unknown, None, None),
    ("", "", Unknown, None, None),
];

/// Le dossier attendu : premier composant, quand le chemin en a plusieurs.
fn folder_of(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    (parts.len() > 1).then(|| parts[0].trim().to_owned())
}

#[test]
fn folder_groups_whatever_the_file_says() {
    let a = parse("Birdmen (01-16) (Tanabe) [Digital-2100]/Birdmen T01 (2020).cbz");
    let b = parse("Birdmen (01-16) (Tanabe) [Digital-2100]/Birdmen Chapitre 2.cbz");
    assert_eq!(a.folder, b.folder);
    assert_eq!(
        a.folder.as_deref(),
        Some("Birdmen (01-16) (Tanabe) [Digital-2100]")
    );
    assert_eq!(parse("Birdmen T01.cbz").folder, None);
}

#[test]
fn table_of_names() {
    assert!(CASES.len() >= 80, "au moins 80 cas (plan de la phase 2)");
    let mut failures = Vec::new();
    for &(path, series, kind, number, number_end) in CASES {
        let got = parse(path);
        let want = ParsedName {
            folder: folder_of(path),
            series: series.to_owned(),
            kind,
            number,
            number_end,
        };
        if got != want {
            failures.push(format!("{path:?}\n  obtenu  {got:?}\n  attendu {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Si ce test casse, les résultats du parseur ont changé : incrémenter `PARSER_VERSION`,
/// puis mettre à jour l'instantané.
#[test]
fn parser_version_is_frozen() {
    let snapshot: Vec<String> = CASES
        .iter()
        .map(|(path, ..)| format!("{:?}", parse(path)))
        .collect();
    let digest = blake3::hash(snapshot.join("\n").as_bytes()).to_hex();
    assert_eq!((PARSER_VERSION, &digest[..16]), (1, "da4a7c7560e2df5a"));
}

#[test]
fn kind_serializes_as_stored_in_db() {
    let names: Vec<String> = [Volume, Chapter, Special, Unknown]
        .iter()
        .map(|k| serde_json::to_string(k).unwrap())
        .collect();
    assert_eq!(
        names,
        [
            r#""volume""#,
            r#""chapter""#,
            r#""special""#,
            r#""unknown""#
        ]
    );
}

#[test]
fn series_order() {
    let mut paths = vec![
        "S/Omake.cbz",
        "S/Chapitre 2.cbz",
        "S/Tome 10.cbz",
        "S/Tomes 1-3.cbz",
        "S/Hors-série 1.cbz",
        "S/Divers.cbz",
        "S/Tome 1.cbz",
        "S/Tome 2.cbz",
        "S/Autre.cbz",
        "S/v2.cbz",
    ];
    paths.sort_by(|a, b| compare_in_series(&parse(a), a, &parse(b), b));
    assert_eq!(
        paths,
        [
            "S/Tome 1.cbz",
            "S/Tomes 1-3.cbz",
            "S/Tome 2.cbz",
            "S/v2.cbz",
            "S/Tome 10.cbz",
            "S/Chapitre 2.cbz",
            "S/Hors-série 1.cbz",
            "S/Omake.cbz",
            "S/Autre.cbz",
            "S/Divers.cbz",
        ]
    );
}

#[test]
fn missing_numbers_order() {
    use std::cmp::Ordering::{Greater, Less};
    assert_eq!(compare_numbers(None, Some(1.0), true), Less);
    assert_eq!(compare_numbers(Some(1.0), None, true), Greater);
    assert_eq!(compare_numbers(None, Some(1.0), false), Greater);
    assert_eq!(compare_numbers(Some(1.0), None, false), Less);
}
