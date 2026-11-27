//! Métadonnées web (lot 5.5b) : recherche et liaison à la demande, relecture des fiches liées,
//! identification des séries en tâche de fond (après un scan, ou depuis l'administration).
//!
//! Rien ne part si l'utilisateur n'a pas activé le réseau (Q33). Les clients sont synchrones :
//! appelés hors du runtime (`spawn_blocking`). Une source qui demande d'attendre ou ne répond
//! plus arrête la tâche (aucune erreur enregistrée série par série) ; la suivante reprendra
//! là où elle s'est arrêtée.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use shiori_db::{Db, Language, MatchedBy, Outcome, Settings};
use shiori_meta::{
    ANILIST_URL, Anilist, COMICVINE_URL, Comicvine, FetchError, MANGADEX_URL, MANGAUPDATES_URL,
    Mangadex, Mangaupdates, Source, SourceId, WIKIPEDIA_URL, WebMetadata, Wikipedia, auto_match,
};
use tokio::task::JoinHandle;

/// Candidats gardés pour une confirmation.
const CANDIDATES: usize = 10;

/// État de l'identification en tâche de fond.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IdentifyStatus {
    pub running: bool,
    /// Séries traitées et à traiter, toutes sources confondues.
    pub done: usize,
    pub total: usize,
    /// Code de la raison d'un arrêt avant la fin (`offline`, `rate_limited`).
    pub stopped: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("métadonnées web désactivées")]
    Disabled,
    #[error("source inconnue")]
    UnknownSource,
    #[error("une identification est déjà en cours")]
    AlreadyRunning,
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error(transparent)]
    Db(#[from] shiori_db::Error),
}

pub struct Metadata {
    db: Arc<Db>,
    sources: Vec<Arc<dyn Source>>,
    /// Adresses de couverture des derniers résultats de recherche, pour relayer leurs vignettes
    /// (les candidats et fiches liées gardent la leur en base).
    covers: Mutex<HashMap<(SourceId, String), String>>,
    running: Mutex<Option<JoinHandle<()>>>,
    status: RwLock<IdentifyStatus>,
}

/// Les vraies sources.
pub fn web_sources() -> Vec<Arc<dyn Source>> {
    vec![
        Arc::new(Anilist::new(ANILIST_URL)),
        Arc::new(Mangadex::new(MANGADEX_URL)),
        Arc::new(Wikipedia::new(WIKIPEDIA_URL)),
        Arc::new(Mangaupdates::new(MANGAUPDATES_URL)),
        Arc::new(Comicvine::new(COMICVINE_URL)),
    ]
}

impl Metadata {
    pub fn new(db: Arc<Db>, sources: Vec<Arc<dyn Source>>) -> Self {
        Self {
            db,
            sources,
            covers: Mutex::default(),
            running: Mutex::new(None),
            status: RwLock::new(IdentifyStatus::default()),
        }
    }

    /// Client d'une source, avec la clé d'API enregistrée (relue à chaque fois : une clé changée
    /// dans l'administration sert tout de suite).
    async fn source(&self, id: SourceId) -> Result<Arc<dyn Source>, MetaError> {
        let client = self
            .sources
            .iter()
            .find(|s| s.id() == id)
            .cloned()
            .ok_or(MetaError::UnknownSource)?;
        client.set_key(self.db.comicvine_key().await?);
        Ok(client)
    }

    async fn enabled(&self) -> Result<(), MetaError> {
        match self.db.settings().await?.metadata_online {
            true => Ok(()),
            false => Err(MetaError::Disabled),
        }
    }

    /// Fiches d'une source qui ressemblent à `query` ; sans adresse de couverture (le
    /// navigateur ne contacte jamais les sources). `lang` : langue de celui qui cherche.
    pub async fn search(
        &self,
        source: SourceId,
        query: String,
        lang: &'static str,
    ) -> Result<Vec<WebMetadata>, MetaError> {
        self.enabled().await?;
        let source = self.source(source).await?;
        let id = source.id();
        let adult = self.db.settings().await?.metadata_adult;
        let mut found = blocking(move || source.search(&query, adult, lang)).await?;
        let mut covers = self.covers.lock().expect("verrou des couvertures");
        // shortcut: oubli en bloc au-delà de 1 000 adresses (une recherche en ajoute 10) ; un vrai LRU si des vignettes manquent en usage réel.
        if covers.len() > 1000 {
            covers.clear();
        }
        for f in &mut found {
            if let Some(url) = f.cover_url.take() {
                covers.insert((id, f.id.clone()), url);
            }
        }
        Ok(found)
    }

    /// Lie une série à une fiche choisie par l'utilisateur, lue aussitôt.
    pub async fn link(
        &self,
        series: i64,
        source: SourceId,
        external_id: String,
    ) -> Result<(), MetaError> {
        self.enabled().await?;
        let client = self.source(source).await?;
        let id = external_id.clone();
        let data = blocking(move || client.fetch(&id)).await?;
        let outcome = Outcome::Linked {
            external_id: data.id.clone(),
            matched_by: MatchedBy::User,
            data: Box::new(data),
        };
        Ok(self.db.record_source(series, source, outcome).await?)
    }

    /// Relit les fiches liées d'une série ; un échec est enregistré, la fiche d'avant reste.
    pub async fn refresh(&self, series: i64) -> Result<(), MetaError> {
        self.enabled().await?;
        let states = self.db.source_states(series).await?;
        for state in states {
            let (Some(external_id), Some(matched_by)) = (state.external_id, state.matched_by)
            else {
                continue;
            };
            let Ok(client) = self.source(state.source).await else {
                continue;
            };
            let outcome = match blocking(move || client.fetch(&external_id)).await {
                Ok(data) => Outcome::Linked {
                    external_id: data.id.clone(),
                    matched_by,
                    data: Box::new(data),
                },
                Err(e) => Outcome::Failed(e.code()),
            };
            self.db.record_source(series, state.source, outcome).await?;
        }
        Ok(())
    }

    /// Vignette d'une fiche (résultat de recherche, candidat ou fiche liée), relayée : le
    /// navigateur ne contacte jamais les sources (lien direct interdit par MangaDex).
    pub async fn thumbnail(
        &self,
        source: SourceId,
        external_id: &str,
    ) -> Result<Vec<u8>, MetaError> {
        self.enabled().await?;
        let client = self.source(source).await?;
        let known = self
            .covers
            .lock()
            .expect("verrou des couvertures")
            .get(&(source, external_id.to_owned()))
            .cloned();
        let url = match known {
            Some(url) => url,
            None => self
                .db
                .cover_url(source, external_id)
                .await?
                .ok_or(FetchError::NotFound)?,
        };
        Ok(blocking(move || client.image(&url)).await?)
    }

    /// Prend la couverture de la fiche liée chez `source` comme couverture de la série (Q32) :
    /// téléchargée, gardée en base, choisie (champ édité `cover_image_id`).
    pub async fn use_cover(&self, series: i64, source: SourceId) -> Result<(), MetaError> {
        self.enabled().await?;
        let client = self.source(source).await?;
        let linked = self.db.linked_sources(series).await?;
        let (_, external_id) = linked
            .into_iter()
            .find(|(s, _)| *s == source)
            .ok_or(FetchError::NotFound)?;
        let url = self
            .db
            .cover_url(source, &external_id)
            .await?
            .ok_or(FetchError::NotFound)?;
        let bytes = blocking(move || client.image(&url)).await?;
        let id = self.db.store_cover(bytes).await?.ok_or_else(|| {
            FetchError::BadResponse("la couverture n'est pas une image lisible".into())
        })?;
        let patch = serde_json::json!({ "cover_image_id": id });
        match self
            .db
            .edit_series(series, patch.as_object().expect("objet").clone())
            .await
        {
            Ok(_) => Ok(()),
            Err(shiori_db::EditError::Db(e)) => Err(e.into()),
            Err(shiori_db::EditError::Invalid(e)) => Err(FetchError::BadResponse(e).into()),
        }
    }

    pub fn status(&self) -> IdentifyStatus {
        self.status.read().expect("verrou de l'état").clone()
    }

    /// Lance l'identification des séries jamais cherchées (ou en échec) ; sans effet si le
    /// réseau est désactivé. Refuse s'il en tourne déjà une.
    pub fn identify(self: &Arc<Self>) -> Result<(), MetaError> {
        let mut running = self.running.lock().expect("verrou de la tâche");
        if running.as_ref().is_some_and(|h| !h.is_finished()) {
            return Err(MetaError::AlreadyRunning);
        }
        *self.status.write().expect("verrou de l'état") = IdentifyStatus {
            running: true,
            ..IdentifyStatus::default()
        };
        let this = self.clone();
        *running = Some(tokio::spawn(async move {
            let stopped = match this.identify_all().await {
                Ok(stopped) => stopped,
                Err(e) => {
                    tracing::error!("identification des séries : {e}");
                    Some("internal".to_owned())
                }
            };
            let mut status = this.status.write().expect("verrou de l'état");
            status.running = false;
            status.stopped = stopped;
        }));
        Ok(())
    }

    /// Rend le code de la raison d'un arrêt avant la fin.
    async fn identify_all(&self) -> Result<Option<String>, MetaError> {
        let settings = self.db.settings().await?;
        if !settings.metadata_online {
            return Ok(None);
        }
        for id in settings.source_order() {
            // Comic Vine seulement à la demande (Q82) : 200 requêtes par heure, clé bloquée si
            // elle va trop vite.
            if id == SourceId::Comicvine {
                continue;
            }
            let Ok(client) = self.source(id).await else {
                continue;
            };
            let todo = self.db.to_identify(id, None).await?;
            self.status.write().expect("verrou de l'état").total += todo.len();
            for series in todo {
                let names = series.names.clone();
                let client = client.clone();
                let adult = settings.metadata_adult;
                let lang = Settings::code(settings.language.unwrap_or(Language::Fr));
                let found = blocking(move || {
                    let found = client.search(&names[0], adult, lang)?;
                    let names: Vec<&str> = names.iter().map(String::as_str).collect();
                    Ok(match auto_match(&names, &found) {
                        // Un résultat de recherche Wikipédia n'a que sa langue, un résultat
                        // MangaUpdates ni auteurs ni statut : la fiche liée est relue entière.
                        Some(i)
                            if matches!(
                                client.id(),
                                SourceId::Wikipedia | SourceId::Mangaupdates
                            ) =>
                        {
                            Ok(client.fetch(&found[i].id)?)
                        }
                        Some(i) => Ok(found[i].clone()),
                        None => Err(found),
                    })
                })
                .await;
                let outcome = match found {
                    Ok(Ok(data)) => Outcome::Linked {
                        external_id: data.id.clone(),
                        matched_by: MatchedBy::Auto,
                        data: Box::new(data),
                    },
                    Ok(Err(found)) => {
                        Outcome::Candidates(found.into_iter().take(CANDIDATES).collect())
                    }
                    // La source demande d'attendre ou ne répond plus : on s'arrête là.
                    Err(e @ (FetchError::RateLimited(_) | FetchError::Offline(_))) => {
                        return Ok(Some(e.code().to_owned()));
                    }
                    Err(e) => Outcome::Failed(e.code()),
                };
                self.db.record_source(series.series_id, id, outcome).await?;
                self.status.write().expect("verrou de l'état").done += 1;
            }
        }
        Ok(None)
    }

    /// Attend la fin de l'identification en cours (tests, arrêt du serveur).
    pub async fn wait(&self) {
        let handle = self.running.lock().expect("verrou de la tâche").take();
        if let Some(handle) = handle {
            let _ = handle.await;
        }
    }
}

/// Appel d'une source hors du runtime asynchrone (clients synchrones, débit par attente).
async fn blocking<T: Send + 'static>(
    call: impl FnOnce() -> Result<T, FetchError> + Send + 'static,
) -> Result<T, FetchError> {
    tokio::task::spawn_blocking(call)
        .await
        .unwrap_or_else(|e| Err(FetchError::BadResponse(format!("appel interrompu : {e}"))))
}

#[cfg(test)]
mod tests;
