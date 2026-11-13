//! Métadonnées locales lues dans l'archive : `ComicInfo.xml` (format ComicRack) et
//! `index.json` (application de téléchargement). Normalisées dans [`LocalMetadata`],
//! sans rien inférer : un champ absent ou vide reste vide.

use serde::{Deserialize, Serialize};

/// Taille maximale d'un fichier de métadonnées lu (un `index.json` liste tous les chapitres).
pub const MAX_METADATA_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataSource {
    Comicinfo,
    IndexJson,
}

impl MetadataSource {
    /// Source d'une entrée de l'archive, d'après son nom : seuls les fichiers à la racine comptent.
    pub fn of_entry(name: &str) -> Option<Self> {
        if name.contains(['/', '\\']) {
            None
        } else if name.eq_ignore_ascii_case("comicinfo.xml") {
            Some(Self::Comicinfo)
        } else if name.eq_ignore_ascii_case("index.json") {
            Some(Self::IndexJson)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    /// Scénario ; ni `story` ni `art` : rôle inconnu.
    pub story: bool,
    pub art: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
// Champs absents = vides : une ligne enregistrée avant l'ajout d'un champ se relit toujours.
#[serde(default)]
pub struct LocalMetadata {
    pub series: Option<String>,
    /// Titre du tome.
    pub title: Option<String>,
    /// Numéro tel qu'écrit dans le fichier : affiché, jamais utilisé pour l'ordre (principe 3).
    pub number: Option<String>,
    pub alt_titles: Vec<String>,
    pub summary: Option<String>,
    pub authors: Vec<Author>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    /// `AAAA`, `AAAA-MM` ou `AAAA-MM-JJ`.
    pub release_date: Option<String>,
    pub publisher: Option<String>,
    /// Code de langue du fichier (`LanguageISO`), celui du résumé.
    pub language: Option<String>,
    pub age_rating: Option<String>,
    /// `ongoing`, `completed`, `hiatus` ou `cancelled`.
    pub status: Option<String>,
    /// `Some(true)` : manga lu de droite à gauche ; `Some(false)` : pas un manga. Une proposition.
    pub right_to_left: Option<bool>,
    pub links: Vec<String>,
}

/// Lit un fichier de métadonnées ; l'erreur est un message lisible (problème du tome).
pub fn parse(source: MetadataSource, bytes: &[u8]) -> Result<LocalMetadata, String> {
    let text = decode(bytes)?;
    match source {
        MetadataSource::Comicinfo => comic_info(&text),
        MetadataSource::IndexJson => index_json(&text),
    }
}

/// UTF-8 (BOM accepté) ou UTF-16 avec BOM, comme en écrivent certains outils Windows.
fn decode(bytes: &[u8]) -> Result<String, String> {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| match be {
                true => u16::from_be_bytes(c),
                false => u16::from_le_bytes(c),
            })
            .collect();
        String::from_utf16(&units).map_err(|_| "texte UTF-16 invalide".to_owned())
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(false),
        [0xFE, 0xFF, ..] => utf16(true),
        _ => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            String::from_utf8(bytes.to_vec()).map_err(|_| "texte UTF-8 invalide".to_owned())
        }
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Liste séparée par des virgules (ComicInfo), sans doublons, ordre conservé.
fn list(value: Option<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in value.iter().flat_map(|v| v.split(',')) {
        let item = item.trim();
        if !item.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(item)) {
            out.push(item.to_owned());
        }
    }
    out
}

fn add_author(authors: &mut Vec<Author>, name: &str, story: bool, art: bool) {
    match authors
        .iter_mut()
        .find(|a| a.name.eq_ignore_ascii_case(name))
    {
        Some(a) => {
            a.story |= story;
            a.art |= art;
        }
        None => authors.push(Author {
            name: name.to_owned(),
            story,
            art,
        }),
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct ComicInfo {
    title: Option<String>,
    series: Option<String>,
    number: Option<String>,
    alternate_series: Option<String>,
    summary: Option<String>,
    year: Option<String>,
    month: Option<String>,
    day: Option<String>,
    writer: Option<String>,
    penciller: Option<String>,
    inker: Option<String>,
    colorist: Option<String>,
    cover_artist: Option<String>,
    publisher: Option<String>,
    genre: Option<String>,
    tags: Option<String>,
    web: Option<String>,
    #[serde(rename = "LanguageISO")]
    language_iso: Option<String>,
    manga: Option<String>,
    age_rating: Option<String>,
}

fn comic_info(text: &str) -> Result<LocalMetadata, String> {
    let info: ComicInfo =
        quick_xml::de::from_str(text).map_err(|e| format!("ComicInfo.xml invalide : {e}"))?;
    let mut authors = Vec::new();
    for name in list(info.writer) {
        add_author(&mut authors, &name, true, false);
    }
    for field in [info.penciller, info.inker, info.colorist, info.cover_artist] {
        for name in list(field) {
            add_author(&mut authors, &name, false, true);
        }
    }
    Ok(LocalMetadata {
        series: clean(info.series),
        title: clean(info.title),
        number: clean(info.number),
        alt_titles: clean(info.alternate_series).into_iter().collect(),
        summary: clean(info.summary),
        authors,
        genres: list(info.genre),
        tags: list(info.tags),
        release_date: date(info.year, info.month, info.day),
        publisher: clean(info.publisher),
        language: clean(info.language_iso),
        age_rating: clean(info.age_rating).filter(|r| r != "Unknown"),
        status: None,
        right_to_left: match clean(info.manga).as_deref() {
            Some("YesAndRightToLeft") => Some(true),
            Some("No") => Some(false),
            _ => None,
        },
        links: urls(info.web),
    })
}

/// Adresses web seulement : certains outils écrivent un titre dans `<Web>`, et un lien
/// affiché ne doit jamais être autre chose que `http(s)` (`javascript:` dans une archive).
fn urls(value: Option<String>) -> Vec<String> {
    let web = |u: &&str| {
        let u = u.to_ascii_lowercase();
        u.starts_with("https://") || u.starts_with("http://")
    };
    value
        .iter()
        .flat_map(|v| v.split_whitespace())
        .filter(web)
        .map(str::to_owned)
        .collect()
}

/// Date partielle valide, sinon rien (ComicRack écrit `-1` pour « inconnu »).
fn date(year: Option<String>, month: Option<String>, day: Option<String>) -> Option<String> {
    let num = |v: Option<String>, max: u32| {
        clean(v)?
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=max).contains(n))
    };
    let year = num(year, 9999)?;
    let Some(month) = num(month, 12) else {
        return Some(format!("{year:04}"));
    };
    Some(match num(day, 31) {
        Some(day) => format!("{year:04}-{month:02}-{day:02}"),
        None => format!("{year:04}-{month:02}"),
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct IndexJson {
    title: Option<String>,
    alt_titles: Vec<String>,
    author: Option<String>,
    authors: Vec<String>,
    description: Option<String>,
    state: Option<String>,
    public_url: Option<String>,
    tags: Vec<IndexTag>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct IndexTag {
    title: Option<String>,
}

fn index_json(text: &str) -> Result<LocalMetadata, String> {
    let index: IndexJson =
        serde_json::from_str(text).map_err(|e| format!("index.json invalide : {e}"))?;
    let mut authors = Vec::new();
    // Plusieurs auteurs dans une même chaîne : « auteur / dessinateur ».
    for name in index.authors.iter().chain(&index.author) {
        for name in name.split(" / ").map(str::trim).filter(|n| !n.is_empty()) {
            add_author(&mut authors, name, false, false);
        }
    }
    Ok(LocalMetadata {
        series: clean(index.title),
        alt_titles: index
            .alt_titles
            .into_iter()
            .filter_map(|t| clean(Some(t)))
            .collect(),
        summary: clean(index.description),
        authors,
        tags: index
            .tags
            .into_iter()
            .filter_map(|t| clean(t.title))
            .collect(),
        status: match clean(index.state).as_deref() {
            Some("ONGOING") => Some("ongoing".to_owned()),
            Some("FINISHED") => Some("completed".to_owned()),
            Some("PAUSED") => Some("hiatus".to_owned()),
            Some("ABANDONED") => Some("cancelled".to_owned()),
            _ => None,
        },
        links: urls(index.public_url),
        ..LocalMetadata::default()
    })
}

#[cfg(test)]
mod tests;
