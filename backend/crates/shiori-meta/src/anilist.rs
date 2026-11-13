//! AniList : API GraphQL publique, sans clé. 30 requêtes par minute en ce moment (90 en temps
//! normal) : une requête toutes les 2 s. Ni collecte massive ni stockage de l'API (conditions
//! d'utilisation) : une recherche à la demande, puis la fiche de la série liée.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::{FetchError, Http, Source, SourceId, Text, WebMetadata, add_author, json, plain};

pub const ANILIST_URL: &str = "https://graphql.anilist.co";

const FIELDS: &str = "id title { romaji english native } synonyms startDate { year } status \
    description(asHtml: false) genres tags { name isMediaSpoiler } siteUrl coverImage { large } \
    staff(perPage: 10) { edges { role node { name { full } } } }";

pub struct Anilist {
    url: String,
    http: Http,
}

impl Anilist {
    /// `url` : [`ANILIST_URL`], ou un faux serveur dans les tests.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            http: Http::new(Duration::from_secs(2)),
        }
    }

    fn query<T: serde::de::DeserializeOwned>(
        &self,
        query: String,
        variables: serde_json::Value,
    ) -> Result<T, FetchError> {
        let body = json!({ "query": query, "variables": variables }).to_string();
        let text = self.http.send(|agent| {
            agent
                .post(&self.url)
                .header("content-type", "application/json")
                .header("accept", "application/json")
                .send(&body)
        })?;
        let answer: Answer<T> = json(&text)?;
        answer
            .data
            .ok_or_else(|| FetchError::BadResponse("réponse sans données".into()))
    }
}

#[derive(Deserialize)]
struct Answer<T> {
    data: Option<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PageData {
    page: Page,
}

#[derive(Deserialize)]
struct Page {
    media: Vec<Media>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaData {
    media: Option<Media>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    id: i64,
    title: Titles,
    #[serde(default)]
    synonyms: Vec<String>,
    start_date: Option<Year>,
    status: Option<String>,
    description: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    tags: Vec<Tag>,
    site_url: Option<String>,
    cover_image: Option<Cover>,
    staff: Option<Staff>,
}

#[derive(Deserialize)]
struct Titles {
    romaji: Option<String>,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Deserialize)]
struct Year {
    year: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tag {
    name: String,
    #[serde(default)]
    is_media_spoiler: bool,
}

#[derive(Deserialize)]
struct Cover {
    large: Option<String>,
}

#[derive(Deserialize)]
struct Staff {
    edges: Vec<Edge>,
}

#[derive(Deserialize)]
struct Edge {
    role: String,
    node: Person,
}

#[derive(Deserialize)]
struct Person {
    name: Name,
}

#[derive(Deserialize)]
struct Name {
    full: Option<String>,
}

impl From<Media> for WebMetadata {
    fn from(m: Media) -> Self {
        let text = |lang: &str, t: Option<String>| {
            t.map(|text| Text {
                lang: Some(lang.into()),
                text,
            })
        };
        let mut titles: Vec<Text> = Vec::new();
        let all = [
            text("ja-ro", m.title.romaji),
            text("en", m.title.english),
            text("ja", m.title.native),
        ]
        .into_iter()
        .flatten()
        .chain(m.synonyms.into_iter().map(|text| Text { lang: None, text }))
        .filter(|t| !t.text.trim().is_empty());
        // Le titre natif répète souvent le romaji (« ONE PIECE ») : chaque texte une fois.
        for t in all {
            if !titles.iter().any(|x| x.text == t.text) {
                titles.push(t);
            }
        }
        let mut authors = Vec::new();
        // « Story & Art », « Story », « Art » ; les assistants et traducteurs ne sont pas des auteurs.
        for edge in m.staff.map(|s| s.edges).unwrap_or_default() {
            let role = edge.role.to_ascii_lowercase();
            // « Original Creator » : l'auteur de l'œuvre adaptée.
            let story = role.contains("story") || role.contains("creator");
            let art = role.contains("art") && !role.contains("assist");
            if let Some(name) = edge.node.name.full.filter(|_| story || art) {
                add_author(&mut authors, &name, story, art);
            }
        }
        WebMetadata {
            id: m.id.to_string(),
            titles,
            // AniList ne décrit qu'en anglais.
            summaries: m
                .description
                .map(|d| plain(&d))
                .filter(|d| !d.is_empty())
                .map(|text| Text {
                    lang: Some("en".into()),
                    text,
                })
                .into_iter()
                .collect(),
            authors,
            genres: m.genres,
            tags: m
                .tags
                .into_iter()
                .filter(|t| !t.is_media_spoiler)
                .map(|t| t.name)
                .collect(),
            year: m.start_date.and_then(|d| d.year),
            status: match m.status.as_deref() {
                Some("RELEASING") => Some("ongoing".into()),
                Some("FINISHED") => Some("completed".into()),
                Some("HIATUS") => Some("hiatus".into()),
                Some("CANCELLED") => Some("cancelled".into()),
                _ => None,
            },
            links: m.site_url.into_iter().collect(),
            cover_url: m.cover_image.and_then(|c| c.large),
        }
    }
}

impl Source for Anilist {
    fn id(&self) -> SourceId {
        SourceId::Anilist
    }

    fn search(&self, query: &str) -> Result<Vec<WebMetadata>, FetchError> {
        // Les romans (`NOVEL`) ne sont pas des livres d'images.
        let q = format!(
            "query ($q: String) {{ Page(perPage: 10) {{ media(search: $q, type: MANGA, format_not: NOVEL) {{ {FIELDS} }} }} }}"
        );
        let data: PageData = self.query(q, json!({ "q": query }))?;
        Ok(data.page.media.into_iter().map(WebMetadata::from).collect())
    }

    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError> {
        let id: i64 = id.parse().map_err(|_| FetchError::NotFound)?;
        let q = format!("query ($id: Int) {{ Media(id: $id, type: MANGA) {{ {FIELDS} }} }}");
        let data: MediaData = self.query(q, json!({ "id": id }))?;
        data.media
            .map(WebMetadata::from)
            .ok_or(FetchError::NotFound)
    }
}
