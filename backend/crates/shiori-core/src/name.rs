//! Analyse du nom d'un tome : série, type et numéro, déduits du chemin relatif à la
//! racine de la bibliothèque. Fonction pure, sans I/O.
//!
//! La série est le premier dossier sous la racine (`Série/Tome 01.cbz`) ; pour un fichier
//! posé à la racine, c'est la partie du nom qui précède le numéro. Le numéro vient d'un
//! marqueur (`Tome`, `Vol`, `Chapitre`, `第1巻`…), ou à défaut d'un nombre nu en fin ou en
//! début de nom. Les tags entre crochets, parenthèses ou accolades sont ignorés.

use std::cmp::Ordering;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::order::compare_entry_names;

/// Version des règles d'analyse : la changer relance l'analyse des emplacements au scan
/// suivant, sans rehash. Toute modification des résultats de [`parse`] doit l'incrémenter.
pub const PARSER_VERSION: u32 = 1;

/// Type d'un fichier dans sa série. L'ordre des variantes est l'ordre d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameKind {
    Volume,
    Chapter,
    Special,
    Unknown,
}

/// Résultat de l'analyse d'un chemin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedName {
    pub series: String,
    pub kind: NameKind,
    /// `1`, `1.5` ; absent si aucun numéro n'a été trouvé.
    pub number: Option<f64>,
    /// Fin d'un intervalle (`Tomes 1-3` → 3), toujours supérieure à `number`.
    pub number_end: Option<f64>,
}

// shortcut: chiffres ASCII seulement (pas de chiffres pleine chasse `１`), à ajouter si une bibliothèque réelle en contient.
const RANGE: &str = r"(?P<n>[0-9]+(?:[.,][0-9]+)?)(?:\s*(?:-|à|to|~)\s*(?:[tv]\.?\s*)?(?P<end>[0-9]+(?:[.,][0-9]+)?))?";

/// Règles essayées dans l'ordre ; `true` = nombre nu, où une année n'est pas un numéro.
/// Un marqueur spécial ne compte que suivi d'un numéro ou en fin de nom : `Extra Life T01` est un tome.
static RULES: LazyLock<Vec<(NameKind, bool, Regex)>> = LazyLock::new(|| {
    let rule = |kind, bare, pattern: &str| {
        let re = Regex::new(&pattern.replace("{R}", RANGE)).expect("règle valide");
        (kind, bare, re)
    };
    vec![
        rule(
            NameKind::Special,
            false,
            r"(?i)\b(?:hors[\s-]?s[ée]rie|hs|sp[ée]cials?|extras?|omake|one[\s-]?shot|int[ée]grale)\b(?:\s*[:#-]?\s*{R}|\s*$)",
        ),
        rule(NameKind::Volume, false, r"(?:第\s*)?{R}\s*巻"),
        rule(NameKind::Chapter, false, r"(?:第\s*)?{R}\s*話"),
        rule(
            NameKind::Volume,
            false,
            r"(?i)\b(?:tomes?|volumes?|vols?|t|v)\s*\.?\s*{R}",
        ),
        rule(
            NameKind::Chapter,
            false,
            r"(?i)(?:\b(?:chapitres?|chapters?|chap|ch|c)\s*\.?\s*|#\s*){R}",
        ),
        rule(NameKind::Volume, true, r"(?:^|[\s-]){R}$"),
        rule(NameKind::Volume, true, r"^{R}(?:\s|$)"),
    ]
});

static TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|\{[^}]*\}").expect("motif valide"));

/// Analyse un chemin relatif à la racine d'une bibliothèque (`/` ou `\` comme séparateur).
pub fn parse(relative_path: &str) -> ParsedName {
    let parts: Vec<&str> = relative_path
        .split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .collect();
    let file = parts.last().copied().unwrap_or("");
    let name = clean(strip_extension(file));
    let folder = (parts.len() > 1).then(|| parts[0].trim().to_owned());

    for (kind, bare, re) in RULES.iter() {
        let Some(caps) = re.captures(&name) else {
            continue;
        };
        let number = caps.name("n").map(|m| to_number(m.as_str()));
        let number_end = caps
            .name("end")
            .map(|m| to_number(m.as_str()))
            .filter(|end| number.is_some_and(|n| *end > n));
        if *bare && number_end.is_none() && is_year(&caps) {
            continue;
        }
        let prefix = name[..caps.get(0).map_or(0, |m| m.start())].trim_matches(SEPARATORS);
        return ParsedName {
            series: folder.unwrap_or_else(|| series_from(prefix, &name)),
            kind: *kind,
            number,
            number_end,
        };
    }
    ParsedName {
        series: folder.unwrap_or_else(|| name.clone()),
        kind: NameKind::Unknown,
        number: None,
        number_end: None,
    }
}

/// Ordre des fichiers dans une série : type, numéro, fin d'intervalle (un tome seul avant
/// un intervalle qui commence au même numéro), puis tri naturel du chemin. Ordre total.
pub fn compare_in_series(a: &ParsedName, a_path: &str, b: &ParsedName, b_path: &str) -> Ordering {
    a.kind
        .cmp(&b.kind)
        .then_with(|| compare_numbers(a.number, b.number, false))
        .then_with(|| compare_numbers(a.number_end, b.number_end, true))
        .then_with(|| compare_entry_names(a_path, a_path.as_bytes(), b_path, b_path.as_bytes()))
}

const SEPARATORS: &[char] = &[' ', '-', '_', ',', ':', '#', '~'];

fn compare_numbers(a: Option<f64>, b: Option<f64>, missing_first: bool) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (None, None) => Ordering::Equal,
        (None, Some(_)) if missing_first => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) if missing_first => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
    }
}

/// Retire l'extension (`.cbz`, `.cbr`…) : un suffixe court qui contient une lettre, pour
/// ne pas couper le `.5` de `Tome 1.5`.
fn strip_extension(file: &str) -> &str {
    match file.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && (1..=4).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic()) =>
        {
            stem
        }
        _ => file,
    }
}

/// Retire les tags, remplace `_` et les points hors nombres décimaux par des espaces
/// (`One.Piece.v01` → `One Piece v01`), puis normalise les espaces.
fn clean(stem: &str) -> String {
    let without_tags = TAGS.replace_all(stem, " ");
    let chars: Vec<char> = without_tags.chars().collect();
    let spaced: String = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let between_digits = i > 0
                && chars[i - 1].is_ascii_digit()
                && chars.get(i + 1).is_some_and(char::is_ascii_digit);
            match c {
                '_' => ' ',
                '.' if !between_digits => ' ',
                other => other,
            }
        })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn series_from(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        prefix.to_owned()
    }
}

fn to_number(text: &str) -> f64 {
    // Les motifs ne capturent que des chiffres et un séparateur décimal : la conversion réussit toujours.
    text.replace(',', ".")
        .parse()
        .expect("nombre capturé valide")
}

fn is_year(caps: &Captures) -> bool {
    caps.name("n").is_some_and(|m| {
        let s = m.as_str();
        s.len() == 4 && (s.starts_with("19") || s.starts_with("20"))
    })
}

#[cfg(test)]
mod tests;
