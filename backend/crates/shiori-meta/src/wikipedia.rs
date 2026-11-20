//! Wikipédia (lot 10.2b) : API Action publique (`/w/api.php`), sans clé. Source de repli : elle
//! n'apporte que le résumé (introduction de l'article, en texte brut) et le lien, jamais titres,
//! genres ni tags. L'article se cherche sur le Wikipédia de la langue de l'interface ; la fiche
//! liée garde aussi l'introduction de l'autre langue, trouvée par les liens interlangues.
//!
//! Conditions (relues le 2026-11-20, `API:Etiquette` et politique User-Agent de Wikimedia) :
//! agent informatif avec un contact, sinon blocage possible (403) ; le contact est l'adresse du
//! projet, jamais l'utilisateur ni l'instance. Requêtes en série, `maxlag` respecté. Textes sous
//! licence CC BY-SA, crédités dans l'interface avec un lien vers l'article.

use std::time::Duration;

use serde::Deserialize;

use crate::{FetchError, Http, Source, SourceId, Text, WebMetadata, json};

/// `{lang}` : `fr` ou `en`. Un faux serveur, sans `{lang}`, dans les tests.
pub const WIKIPEDIA_URL: &str = "https://{lang}.wikipedia.org";
const AGENT: &str = concat!(
    "Shiori/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/befa160/shiori)"
);
/// Seules langues demandées : aucun autre hôte n'est jamais construit à partir d'un identifiant.
const LANGS: [&str; 2] = ["fr", "en"];
/// Une description Wikidata, quand l'article en a une, doit parler de bande dessinée : écarte le
/// jeu vidéo ou le film qui porte le même titre.
const COMICS: [&str; 9] = [
    "manga",
    "manhwa",
    "manhua",
    "webtoon",
    "bande dessinée",
    "bandes dessinées",
    "comic",
    "roman graphique",
    "graphic novel",
];

pub struct Wikipedia {
    url: String,
    http: Http,
}

impl Wikipedia {
    /// `url` : [`WIKIPEDIA_URL`], ou un faux serveur dans les tests.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            http: Http::with_agent(Duration::from_millis(500), AGENT),
        }
    }

    fn query(&self, lang: &str, params: &[(&str, &str)]) -> Result<Vec<Page>, FetchError> {
        let url = format!("{}/w/api.php", self.url.replace("{lang}", lang));
        let body = self.http.send(|agent| {
            let mut request = agent.get(&url);
            let common = [
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
            ];
            // Serveurs en retard de réplication : l'API répond `maxlag`, on attendra.
            for (key, value) in common.iter().chain(params).chain(&[("maxlag", "5")]) {
                request = request.query(*key, *value);
            }
            request.call()
        })?;
        let answer: Answer = json(&body)?;
        if let Some(error) = answer.error {
            return Err(match error.code.as_str() {
                "maxlag" | "ratelimited" => FetchError::RateLimited(Duration::from_secs(5)),
                _ => FetchError::BadResponse(error.code),
            });
        }
        let mut pages = answer.query.map(|q| q.pages).unwrap_or_default();
        pages.sort_by_key(|p| p.index);
        Ok(pages)
    }
}

#[derive(Deserialize)]
struct Answer {
    query: Option<Query>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    code: String,
}

#[derive(Deserialize)]
struct Query {
    #[serde(default)]
    pages: Vec<Page>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Page {
    title: String,
    /// Rang dans les résultats d'une recherche.
    index: u32,
    missing: bool,
    extract: Option<String>,
    description: Option<String>,
    pageprops: Option<PageProps>,
    fullurl: Option<String>,
    langlinks: Vec<LangLink>,
}

#[derive(Deserialize, Default)]
struct PageProps {
    disambiguation: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct LangLink {
    title: String,
}

/// Le titre sans la précision entre parenthèses : « Dofus (manga) » → « Dofus ».
fn bare(title: &str) -> &str {
    match title.rsplit_once(" (") {
        Some((head, tail)) if tail.ends_with(')') => head,
        _ => title,
    }
}

fn summary(page: &Page, lang: &str) -> Option<Text> {
    let text = page.extract.as_deref()?.trim();
    (!text.is_empty()).then(|| Text {
        lang: Some(lang.to_owned()),
        text: text.to_owned(),
    })
}

/// `fr:One Piece` → (`fr`, `One Piece`), langue connue seulement.
fn split_id(id: &str) -> Option<(&str, &str)> {
    let (lang, title) = id.split_once(':')?;
    (LANGS.contains(&lang) && !title.trim().is_empty()).then_some((lang, title))
}

impl Source for Wikipedia {
    fn id(&self) -> SourceId {
        SourceId::Wikipedia
    }

    /// Articles du Wikipédia de `lang` (anglais si la langue n'est pas prise en charge), sans les
    /// pages d'homonymie ni ce qui n'est visiblement pas une bande dessinée. Titre (pour le
    /// rapprochement et la fenêtre « Identifier »), introduction et lien ; pas de couverture.
    fn search(&self, query: &str, _: bool, lang: &str) -> Result<Vec<WebMetadata>, FetchError> {
        let lang = if LANGS.contains(&lang) { lang } else { "en" };
        let pages = self.query(
            lang,
            &[
                ("generator", "search"),
                ("gsrsearch", query),
                ("gsrnamespace", "0"),
                ("gsrlimit", "10"),
                ("prop", "extracts|description|pageprops|info"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("exlimit", "max"),
                ("ppprop", "disambiguation"),
                ("inprop", "url"),
            ],
        )?;
        Ok(pages
            .into_iter()
            .filter(|p| {
                p.pageprops
                    .as_ref()
                    .is_none_or(|pp| pp.disambiguation.is_none())
            })
            .filter(|p| {
                p.description.as_deref().is_none_or(|d| {
                    let d = d.to_lowercase();
                    COMICS.iter().any(|c| d.contains(c))
                })
            })
            .map(|p| WebMetadata {
                id: format!("{lang}:{}", p.title),
                titles: vec![Text {
                    lang: Some(lang.to_owned()),
                    text: bare(&p.title).to_owned(),
                }],
                summaries: summary(&p, lang).into_iter().collect(),
                links: p.fullurl.into_iter().collect(),
                ..WebMetadata::default()
            })
            .collect())
    }

    /// L'article `lang:Titre` et, s'il en a un, son équivalent dans l'autre langue : les deux
    /// introductions avec leur langue, les deux liens. Sans titres : Wikipédia ne nomme pas la série.
    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError> {
        let (lang, title) = split_id(id).ok_or(FetchError::NotFound)?;
        let other = LANGS.into_iter().find(|l| *l != lang).unwrap_or("en");
        let page = self
            .query(
                lang,
                &[
                    ("titles", title),
                    ("redirects", "1"),
                    ("prop", "extracts|langlinks|info"),
                    ("exintro", "1"),
                    ("explaintext", "1"),
                    ("lllang", other),
                    ("inprop", "url"),
                ],
            )?
            .into_iter()
            .next()
            .filter(|p| !p.missing)
            .ok_or(FetchError::NotFound)?;
        let mut data = WebMetadata {
            id: id.to_owned(),
            summaries: summary(&page, lang).into_iter().collect(),
            links: page.fullurl.clone().into_iter().collect(),
            ..WebMetadata::default()
        };
        if let Some(link) = page.langlinks.first() {
            let params = [
                ("titles", link.title.as_str()),
                ("prop", "extracts|info"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("inprop", "url"),
            ];
            if let Some(page) = self.query(other, &params)?.into_iter().find(|p| !p.missing) {
                data.summaries.extend(summary(&page, other));
                data.links.extend(page.fullurl);
            }
        }
        Ok(data)
    }

    /// Pas de couverture chez Wikipédia.
    fn image(&self, _: &str) -> Result<Vec<u8>, FetchError> {
        Err(FetchError::NotFound)
    }
}
