//! Métadonnées web (lot 5.5) : AniList et MangaDex, normalisées dans [`WebMetadata`].
//!
//! Rien de personnel ne part : seul le titre cherché (ou l'identifiant d'une fiche) est envoyé,
//! avec un agent générique `Shiori/<version>`. Chaque source a son débit (AniList : 30 requêtes
//! par minute en ce moment, MangaDex : 5 par seconde) ; une source injoignable est une erreur
//! typée, jamais une panique. Clients synchrones : à appeler hors du runtime asynchrone.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
pub use shiori_core::Author;

mod anilist;
mod mangadex;

pub use anilist::{ANILIST_URL, Anilist};
pub use mangadex::{MANGADEX_URL, Mangadex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    Anilist,
    Mangadex,
}

/// Un texte et sa langue (`en`, `fr`, `ja`, `ja-ro` pour le romaji…) ; `None` : inconnue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub lang: Option<String>,
    pub text: String,
}

/// Fiche d'une série chez une source, normalisée.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebMetadata {
    /// Identifiant chez la source.
    pub id: String,
    /// Titre principal d'abord, puis les autres.
    pub titles: Vec<Text>,
    pub summaries: Vec<Text>,
    pub authors: Vec<Author>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub year: Option<i32>,
    /// `ongoing`, `completed`, `hiatus` ou `cancelled`.
    pub status: Option<String>,
    /// Page de la fiche chez la source.
    pub links: Vec<String>,
    /// Couverture chez la source : à télécharger par le serveur, jamais à donner au navigateur.
    pub cover_url: Option<String>,
}

/// Erreur d'une source ; `code` est stable (enregistré en base, traduit par l'interface).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FetchError {
    #[error("source injoignable : {0}")]
    Offline(String),
    #[error("trop de requêtes, réessayer dans {0:?}")]
    RateLimited(Duration),
    #[error("fiche introuvable")]
    NotFound,
    #[error("réponse HTTP {0}")]
    Http(u16),
    #[error("réponse inattendue : {0}")]
    BadResponse(String),
}

impl FetchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Offline(_) => "offline",
            Self::RateLimited(_) => "rate_limited",
            Self::NotFound => "not_found",
            Self::Http(_) => "http_error",
            Self::BadResponse(_) => "bad_response",
        }
    }
}

pub trait Source: Send + Sync {
    fn id(&self) -> SourceId;
    /// Séries dont un titre ressemble à `query`, les plus pertinentes d'abord.
    fn search(&self, query: &str) -> Result<Vec<WebMetadata>, FetchError>;
    fn fetch(&self, id: &str) -> Result<WebMetadata, FetchError>;
    /// Image d'une fiche (couverture), depuis les seuls hôtes de la source.
    fn image(&self, url: &str) -> Result<Vec<u8>, FetchError>;
}

type Response = Result<ureq::http::Response<ureq::Body>, ureq::Error>;

/// Attente au-delà de laquelle une requête renonce plutôt que de faire patienter (429 reçu).
const MAX_WAIT: Duration = Duration::from_secs(10);

/// Client HTTP d'une source : agent générique, délai global, débit minimal entre deux requêtes.
struct Http {
    agent: ureq::Agent,
    gap: Duration,
    /// Instant avant lequel aucune requête ne part.
    next: Mutex<Instant>,
}

impl Http {
    fn new(gap: Duration) -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent(concat!("Shiori/", env!("CARGO_PKG_VERSION")))
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            // Aucune redirection suivie : une image redirigée vers le réseau local contournerait
            // la liste blanche des hôtes (SSRF) ; un 3xx devient une erreur `http_error`.
            .max_redirects(0)
            .build()
            .into();
        Self {
            agent,
            gap,
            next: Mutex::new(Instant::now()),
        }
    }

    /// Attend son tour ; renonce si une source a demandé d'attendre plus de [`MAX_WAIT`].
    fn turn(&self) -> Result<(), FetchError> {
        let mut next = self.next.lock().expect("verrou du débit");
        let now = Instant::now();
        let wait = next.saturating_duration_since(now);
        if wait > MAX_WAIT {
            return Err(FetchError::RateLimited(wait));
        }
        std::thread::sleep(wait);
        *next = now.max(*next) + self.gap;
        Ok(())
    }

    /// Envoie la requête et rend le corps (texte) d'une réponse 200.
    fn send(&self, request: impl FnOnce(&ureq::Agent) -> Response) -> Result<String, FetchError> {
        self.ok(request)?
            .body_mut()
            .read_to_string()
            .map_err(|e| FetchError::BadResponse(e.to_string()))
    }

    /// Télécharge une image, `https` seulement et depuis un hôte que `allowed` accepte : une
    /// adresse venue d'une réponse ne fait jamais appeler le réseau local (SSRF). 8 Mio au plus.
    fn image(&self, url: &str, allowed: fn(&str) -> bool) -> Result<Vec<u8>, FetchError> {
        if !image_host(url).is_some_and(|h| allowed(&h)) {
            return Err(FetchError::BadResponse(format!(
                "adresse d'image refusée : {url}"
            )));
        }
        self.ok(|agent| agent.get(url).call())?
            .body_mut()
            .with_config()
            .limit(8 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| FetchError::BadResponse(e.to_string()))
    }

    /// Envoie la requête (à son tour) et rend une réponse 200 ; erreur typée sinon.
    fn ok(
        &self,
        request: impl FnOnce(&ureq::Agent) -> Response,
    ) -> Result<ureq::http::Response<ureq::Body>, FetchError> {
        self.turn()?;
        let response = request(&self.agent).map_err(|e| FetchError::Offline(e.to_string()))?;
        match response.status().as_u16() {
            200 => Ok(response),
            404 => Err(FetchError::NotFound),
            429 => {
                // `Retry-After` en secondes ; une minute sans lui (le blocage d'AniList).
                let secs = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()?.trim().parse().ok())
                    .unwrap_or(60);
                let wait = Duration::from_secs(secs);
                *self.next.lock().expect("verrou du débit") = Instant::now() + wait;
                Err(FetchError::RateLimited(wait))
            }
            status => Err(FetchError::Http(status)),
        }
    }
}

fn json<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, FetchError> {
    serde_json::from_str(body).map_err(|e| FetchError::BadResponse(e.to_string()))
}

/// Hôte d'une adresse `https` (en minuscules) ; `None` pour tout autre schéma, ou si
/// l'adresse porte des identifiants (`user@hôte`), un port ou une barre inverse.
fn image_host(url: &str) -> Option<String> {
    let host = url
        .strip_prefix("https://")?
        .split(['/', '?', '#'])
        .next()?;
    (!host.is_empty() && !host.contains(['@', ':', '\\'])).then(|| host.to_ascii_lowercase())
}

/// Minuscules, sans accents ni ponctuation ni espaces : « One-Piece ! » = « one piece ».
pub fn normalize(title: &str) -> String {
    title
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' | 'ā' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' | 'ē' => Some('e'),
            'î' | 'ï' | 'í' | 'ì' | 'ī' => Some('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'ō' => Some('o'),
            'û' | 'ü' | 'ú' | 'ù' | 'ū' => Some('u'),
            'ç' => Some('c'),
            'ñ' => Some('n'),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// Rapprochement automatique (Q30) : le seul candidat dont un titre égale un des noms de la
/// série, une fois normalisés ; aucun si plusieurs ou aucun ne correspondent (à confirmer).
/// D'abord parmi les titres principaux (langue connue), puis parmi tous : un synonyme d'une
/// parodie (« Wan Piece », alias « One Piece » chez AniList) ne bloque pas la vraie série.
pub fn auto_match(names: &[&str], candidates: &[WebMetadata]) -> Option<usize> {
    let names: Vec<String> = names
        .iter()
        .map(|n| normalize(n))
        .filter(|n| !n.is_empty())
        .collect();
    let unique = |main_only: bool| {
        let mut found = candidates.iter().enumerate().filter(|(_, c)| {
            c.titles
                .iter()
                .any(|t| (!main_only || t.lang.is_some()) && names.contains(&normalize(&t.text)))
        });
        let first = found.next()?.0;
        found.next().is_none().then_some(first)
    };
    unique(true).or_else(|| unique(false))
}

/// Texte sans balises HTML (`<br>` devient un retour à la ligne) et sans entités courantes.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        if rest[start + 1..start + end]
            .trim_start_matches('/')
            .to_ascii_lowercase()
            .starts_with("br")
        {
            out.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    let out = out
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    // Lignes vides en trop (les `<br><br>` encadrés de retours à la ligne).
    let mut lines: Vec<&str> = Vec::new();
    for line in out.lines().map(str::trim) {
        if !(line.is_empty() && lines.last().is_none_or(|l| l.is_empty())) {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_owned()
}

/// Ajoute un auteur, ou ses rôles s'il est déjà là.
fn add_author(authors: &mut Vec<Author>, name: &str, story: bool, art: bool) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    match authors.iter_mut().find(|a| a.name == name) {
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

#[cfg(test)]
mod tests;
