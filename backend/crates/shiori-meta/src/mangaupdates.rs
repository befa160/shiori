//! MangaUpdates (version 1.4, lot 1.4c) : API REST publique, sans clé. Politique d'usage :
//! créditer MangaUpdates, espacer les requêtes et garder les réponses (les fiches liées sont en
//! base, relues seulement à la demande) ; aucun débit chiffré : une requête par seconde.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::{FetchError, Http, Source, SourceId, Text, WebMetadata, add_author, json, plain};

pub const MANGAUPDATES_URL: &str = "https://api.mangaupdates.com/v1";
/// Genres des fiches pour adultes, exclus de la recherche sauf réglage contraire.
const ADULT: [&str; 3] = ["Adult", "Hentai", "Smut"];

pub struct Mangaupdates {
    url: String,
    http: Http,
}

impl Mangaupdates {
    /// `url` : [`MANGAUPDATES_URL`], ou un faux serveur dans les tests.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            http: Http::new(Duration::from_secs(1)),
        }
    }
}

#[derive(Deserialize)]
struct Search {
    results: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    record: Series,
    /// Titre qui a répondu à la recherche (souvent un titre associé, absent de `record`).
    hit_title: Option<String>,
}

#[derive(Deserialize)]
struct Series {
    series_id: i64,
    title: String,
    #[serde(default)]
    associated: Vec<Associated>,
    description: Option<String>,
    year: Option<String>,
    /// Texte libre : « 7 Volumes (Complete) », « 12 Volumes (Ongoing) »…
    status: Option<String>,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default)]
    authors: Vec<Person>,
    url: Option<String>,
    image: Option<Image>,
}

#[derive(Deserialize)]
struct Associated {
    title: String,
}

#[derive(Deserialize)]
struct Genre {
    genre: String,
}

#[derive(Deserialize)]
struct Person {
    name: String,
    /// `Author` (scénario) ou `Artist` (dessin).
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct Image {
    url: ImageUrl,
}

#[derive(Deserialize)]
struct ImageUrl {
    original: Option<String>,
}

/// Statut de parution d'après la première ligne du texte de MangaUpdates (les suivantes parlent
/// d'autres parties ou d'éditions).
fn status(text: &str) -> Option<String> {
    let first = text.lines().next()?.to_ascii_lowercase();
    // « Incomplete » contient « complete » : les autres statuts d'abord.
    let status = if first.contains("ongoing") {
        "ongoing"
    } else if first.contains("hiatus") {
        "hiatus"
    } else if first.contains("cancel") || first.contains("discontinued") {
        "cancelled"
    } else if first.contains("complete") && !first.contains("incomplete") {
        "completed"
    } else {
        return None;
    };
    Some(status.into())
}

impl Series {
    fn into_metadata(self, hit_title: Option<String>) -> WebMetadata {
        // Langue des titres inconnue : le principal est le plus souvent romanisé, les associés
        // mêlent toutes les langues.
        let mut titles = vec![Text {
            lang: None,
            text: self.title,
        }];
        for title in hit_title
            .into_iter()
            .chain(self.associated.into_iter().map(|a| a.title))
        {
            if !titles.iter().any(|t| t.text == title) {
                titles.push(Text {
                    lang: None,
                    text: title,
                });
            }
        }
        let mut authors = Vec::new();
        for person in &self.authors {
            let story = person.kind == "Author";
            add_author(&mut authors, &person.name, story, !story);
        }
        WebMetadata {
            id: self.series_id.to_string(),
            titles,
            summaries: self
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
            genres: self.genres.into_iter().map(|g| g.genre).collect(),
            tags: Vec::new(),
            year: self.year.and_then(|y| y.trim().parse().ok()),
            status: self.status.as_deref().and_then(status),
            links: self.url.into_iter().collect(),
            cover_url: self.image.and_then(|i| i.url.original),
        }
    }
}

impl Source for Mangaupdates {
    fn id(&self) -> SourceId {
        SourceId::Mangaupdates
    }

    /// Les résultats n'ont ni auteurs ni statut : la fiche liée est relue entière.
    fn search(&self, query: &str, adult: bool, _: &str) -> Result<Vec<WebMetadata>, FetchError> {
        let mut body = json!({ "search": query, "perpage": 10 });
        if !adult {
            body["exclude_genre"] = json!(ADULT);
        }
        let body = body.to_string();
        let url = format!("{}/series/search", self.url);
        let text = self.http.send(|agent| {
            agent
                .post(&url)
                .header("content-type", "application/json")
                .header("accept", "application/json")
                .send(&body)
        })?;
        let found: Search = json(&text)?;
        Ok(found
            .results
            .into_iter()
            .map(|hit| hit.record.into_metadata(hit.hit_title))
            .collect())
    }

    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError> {
        // Un identifiant est un nombre : rien d'autre ne part dans le chemin.
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(FetchError::NotFound);
        }
        let url = format!("{}/series/{id}", self.url);
        let text = self
            .http
            .send(|agent| agent.get(&url).header("accept", "application/json").call())?;
        let series: Series = json(&text)?;
        Ok(series.into_metadata(None))
    }

    fn image(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        self.http.image(url, |host| host == "cdn.mangaupdates.com")
    }
}

#[cfg(test)]
mod tests;
