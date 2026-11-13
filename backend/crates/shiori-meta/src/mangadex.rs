//! MangaDex : API REST publique, sans clé. Environ 5 requêtes par seconde par adresse IP : une
//! toutes les 250 ms. Conditions : agent honnête, pas de lien direct vers ses images (le serveur
//! les relaie), MangaDex crédité dans l'interface, pas de publicité.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::{FetchError, Http, Source, SourceId, Text, WebMetadata, add_author, json};

pub const MANGADEX_URL: &str = "https://api.mangadex.org";
const COVERS: &str = "https://uploads.mangadex.org/covers";
const INCLUDES: [(&str, &str); 3] = [
    ("includes[]", "author"),
    ("includes[]", "artist"),
    ("includes[]", "cover_art"),
];

pub struct Mangadex {
    url: String,
    http: Http,
}

impl Mangadex {
    /// `url` : [`MANGADEX_URL`], ou un faux serveur dans les tests.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            http: Http::new(Duration::from_millis(250)),
        }
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<String, FetchError> {
        let url = format!("{}{path}", self.url);
        self.http.send(|agent| {
            let mut request = agent.get(&url).header("accept", "application/json");
            for (key, value) in query.iter().chain(&INCLUDES) {
                request = request.query(*key, *value);
            }
            request.call()
        })
    }
}

#[derive(Deserialize)]
struct List {
    data: Vec<Manga>,
}

#[derive(Deserialize)]
struct One {
    data: Manga,
}

#[derive(Deserialize)]
struct Manga {
    id: String,
    attributes: Attributes,
    #[serde(default)]
    relationships: Vec<Relationship>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attributes {
    #[serde(default)]
    title: HashMap<String, String>,
    #[serde(default)]
    alt_titles: Vec<HashMap<String, String>>,
    #[serde(default)]
    description: HashMap<String, String>,
    year: Option<i32>,
    status: Option<String>,
    #[serde(default)]
    tags: Vec<TagEntry>,
}

#[derive(Deserialize)]
struct TagEntry {
    attributes: TagAttributes,
}

#[derive(Deserialize)]
struct TagAttributes {
    #[serde(default)]
    name: HashMap<String, String>,
    group: String,
}

#[derive(Deserialize)]
struct Relationship {
    #[serde(rename = "type")]
    kind: String,
    attributes: Option<RelationshipAttributes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipAttributes {
    name: Option<String>,
    file_name: Option<String>,
}

/// Textes d'un dictionnaire langue → texte, dans un ordre stable (par langue).
fn texts(map: HashMap<String, String>) -> Vec<Text> {
    let mut out: Vec<Text> = map
        .into_iter()
        .filter(|(_, t)| !t.trim().is_empty())
        .map(|(lang, text)| Text {
            lang: Some(lang),
            text: text.trim().to_owned(),
        })
        .collect();
    out.sort_by(|a, b| a.lang.cmp(&b.lang));
    out
}

impl From<Manga> for WebMetadata {
    fn from(m: Manga) -> Self {
        let a = m.attributes;
        let mut titles = texts(a.title);
        for alt in a.alt_titles {
            for t in texts(alt) {
                if !titles.iter().any(|x| x.text == t.text) {
                    titles.push(t);
                }
            }
        }
        let mut authors = Vec::new();
        let mut cover = None;
        for r in m.relationships {
            let Some(attr) = r.attributes else { continue };
            match (r.kind.as_str(), attr.name, attr.file_name) {
                ("author", Some(name), _) => add_author(&mut authors, &name, true, false),
                ("artist", Some(name), _) => add_author(&mut authors, &name, false, true),
                ("cover_art", _, Some(file)) => {
                    cover = Some(format!("{COVERS}/{}/{file}.512.jpg", m.id))
                }
                _ => {}
            }
        }
        let tags = |group: &str| -> Vec<String> {
            a.tags
                .iter()
                .filter(|t| t.attributes.group == group)
                .filter_map(|t| t.attributes.name.get("en").cloned())
                .collect()
        };
        WebMetadata {
            titles,
            summaries: texts(a.description),
            authors,
            genres: tags("genre"),
            tags: tags("theme"),
            year: a.year,
            status: a
                .status
                .filter(|s| ["ongoing", "completed", "hiatus", "cancelled"].contains(&s.as_str())),
            links: vec![format!("https://mangadex.org/title/{}", m.id)],
            cover_url: cover,
            id: m.id,
        }
    }
}

impl Source for Mangadex {
    fn id(&self) -> SourceId {
        SourceId::Mangadex
    }

    fn search(&self, query: &str) -> Result<Vec<WebMetadata>, FetchError> {
        let body = self.get(
            "/manga",
            &[
                ("title", query),
                ("limit", "10"),
                ("order[relevance]", "desc"),
            ],
        )?;
        Ok(json::<List>(&body)?
            .data
            .into_iter()
            .map(WebMetadata::from)
            .collect())
    }

    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError> {
        // Un identifiant MangaDex est un UUID : rien d'autre ne part dans l'adresse.
        if id.len() != 36 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return Err(FetchError::NotFound);
        }
        let body = self.get(&format!("/manga/{id}"), &[])?;
        Ok(json::<One>(&body)?.data.into())
    }
}
