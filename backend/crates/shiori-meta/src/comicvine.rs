//! Comic Vine (version 1.4, lot 1.4d) : comics et BD ; un « volume » chez Comic Vine est une
//! série chez Shiori. Clé d'API personnelle, gratuite, une par installation (« a unique api key
//! for each site »), saisie par l'administrateur ; usage non commercial, Comic Vine crédité.
//! 200 requêtes par ressource et par heure, et blocage d'une clé trop rapide : la source ne sert
//! qu'à la demande (Q82), jamais à l'identification automatique, une requête toutes les 2 s.

use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;

use crate::{FetchError, Http, Source, SourceId, Text, WebMetadata, json, plain};

pub const COMICVINE_URL: &str = "https://comicvine.gamespot.com/api";
const FIELDS: &str = "id,name,aliases,deck,description,start_year,image,site_detail_url";

pub struct Comicvine {
    url: String,
    http: Http,
    /// Clé de l'administrateur, relue des réglages avant chaque appel ; `None` : source inactive.
    key: Mutex<Option<String>>,
}

impl Comicvine {
    /// `url` : [`COMICVINE_URL`], ou un faux serveur dans les tests.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            http: Http::new(Duration::from_secs(2)),
            key: Mutex::new(None),
        }
    }

    /// `GET {url}{path}` avec la clé ; la clé n'apparaît jamais dans une erreur (elles remontent
    /// jusqu'à l'interface et au journal).
    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<String, FetchError> {
        let key = self
            .key
            .lock()
            .expect("verrou de la clé")
            .clone()
            .ok_or(FetchError::MissingKey)?;
        let url = format!("{}{path}", self.url);
        let hide = |text: String| text.replace(&key, "…");
        let text = self
            .http
            .send(|agent| {
                let mut request = agent
                    .get(&url)
                    .header("accept", "application/json")
                    .query("api_key", &key)
                    .query("format", "json")
                    .query("field_list", FIELDS);
                for (name, value) in query {
                    request = request.query(*name, *value);
                }
                request.call()
            })
            .map_err(|e| match e {
                FetchError::Http(401) => FetchError::InvalidKey,
                FetchError::Offline(m) => FetchError::Offline(hide(m)),
                FetchError::BadResponse(m) => FetchError::BadResponse(hide(m)),
                e => e,
            })?;
        Ok(text)
    }
}

/// Enveloppe de toute réponse : `status_code` 1 si tout va bien.
#[derive(Deserialize)]
struct Answer<T> {
    status_code: i32,
    #[serde(default)]
    error: String,
    results: Option<T>,
}

impl<T> Answer<T> {
    fn results(self) -> Result<T, FetchError> {
        match self.status_code {
            1 => self
                .results
                .ok_or_else(|| FetchError::BadResponse("réponse sans résultats".into())),
            100 => Err(FetchError::InvalidKey),
            101 => Err(FetchError::NotFound),
            _ => Err(FetchError::BadResponse(self.error)),
        }
    }
}

#[derive(Deserialize)]
struct Volume {
    id: i64,
    name: Option<String>,
    /// Un alias par ligne.
    aliases: Option<String>,
    /// Résumé court.
    deck: Option<String>,
    /// Article complet, en HTML (souvent des listes d'éditions) : seulement sans `deck`.
    description: Option<String>,
    /// Texte ou nombre selon les fiches.
    start_year: Option<serde_json::Value>,
    image: Option<Image>,
    site_detail_url: Option<String>,
}

#[derive(Deserialize)]
struct Image {
    original_url: Option<String>,
}

impl From<Volume> for WebMetadata {
    fn from(v: Volume) -> Self {
        let mut titles: Vec<Text> = Vec::new();
        for (i, title) in v
            .name
            .into_iter()
            .chain(v.aliases.iter().flat_map(|a| a.lines().map(str::to_owned)))
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .enumerate()
        {
            if !titles.iter().any(|t| t.text == title) {
                // Le nom est anglais (ou celui de l'édition américaine) ; les alias, de tout.
                let lang = (i == 0).then(|| "en".to_owned());
                titles.push(Text { lang, text: title });
            }
        }
        let summary = v
            .deck
            .map(|d| plain(&d))
            .filter(|d| !d.is_empty())
            .or_else(|| v.description.map(|d| plain(&d)).filter(|d| !d.is_empty()));
        WebMetadata {
            id: v.id.to_string(),
            titles,
            summaries: summary
                .map(|text| Text {
                    lang: Some("en".into()),
                    text,
                })
                .into_iter()
                .collect(),
            year: v.start_year.and_then(|y| match y {
                serde_json::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
                serde_json::Value::String(s) => s.trim().parse().ok(),
                _ => None,
            }),
            links: v.site_detail_url.into_iter().collect(),
            cover_url: v.image.and_then(|i| i.original_url),
            ..WebMetadata::default()
        }
    }
}

impl Source for Comicvine {
    fn id(&self) -> SourceId {
        SourceId::Comicvine
    }

    fn set_key(&self, key: Option<String>) {
        *self.key.lock().expect("verrou de la clé") = key;
    }

    /// Pas de fiches pour adultes à exclure : Comic Vine n'en signale pas.
    fn search(&self, query: &str, _: bool, _: &str) -> Result<Vec<WebMetadata>, FetchError> {
        let text = self.get(
            "/search/",
            &[("resources", "volume"), ("query", query), ("limit", "10")],
        )?;
        let answer: Answer<Vec<Volume>> = json(&text)?;
        Ok(answer.results()?.into_iter().map(Into::into).collect())
    }

    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(FetchError::NotFound);
        }
        let text = self.get(&format!("/volume/4050-{id}/"), &[])?;
        let answer: Answer<Volume> = json(&text)?;
        Ok(answer.results()?.into())
    }

    fn image(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        self.http
            .image(url, |host| host == "comicvine.gamespot.com")
    }
}
