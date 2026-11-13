//! Métadonnées web (lot 5.5b) : recherche et liaison à la demande, relecture des fiches liées,
//! identification des séries en tâche de fond (après un scan, ou depuis l'administration).
//!
//! Rien ne part si l'utilisateur n'a pas activé le réseau (Q33). Les clients sont synchrones :
//! appelés hors du runtime (`spawn_blocking`). Une source qui demande d'attendre ou ne répond
//! plus arrête la tâche (aucune erreur enregistrée série par série) ; la suivante reprendra
//! là où elle s'est arrêtée.

use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use shiori_db::{Db, MatchedBy, Outcome};
use shiori_meta::{
    ANILIST_URL, Anilist, FetchError, MANGADEX_URL, Mangadex, Source, SourceId, WebMetadata,
    auto_match,
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
    running: Mutex<Option<JoinHandle<()>>>,
    status: RwLock<IdentifyStatus>,
}

/// Les vraies sources.
pub fn web_sources() -> Vec<Arc<dyn Source>> {
    vec![
        Arc::new(Anilist::new(ANILIST_URL)),
        Arc::new(Mangadex::new(MANGADEX_URL)),
    ]
}

impl Metadata {
    pub fn new(db: Arc<Db>, sources: Vec<Arc<dyn Source>>) -> Self {
        Self {
            db,
            sources,
            running: Mutex::new(None),
            status: RwLock::new(IdentifyStatus::default()),
        }
    }

    fn source(&self, id: SourceId) -> Result<Arc<dyn Source>, MetaError> {
        self.sources
            .iter()
            .find(|s| s.id() == id)
            .cloned()
            .ok_or(MetaError::UnknownSource)
    }

    async fn enabled(&self) -> Result<(), MetaError> {
        match self.db.settings().await?.metadata_online {
            true => Ok(()),
            false => Err(MetaError::Disabled),
        }
    }

    /// Fiches d'une source qui ressemblent à `query` ; sans adresse de couverture (le
    /// navigateur ne contacte jamais les sources).
    pub async fn search(
        &self,
        source: SourceId,
        query: String,
    ) -> Result<Vec<WebMetadata>, MetaError> {
        self.enabled().await?;
        let source = self.source(source)?;
        let mut found = blocking(move || source.search(&query)).await?;
        for f in &mut found {
            f.cover_url = None;
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
        let client = self.source(source)?;
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
            let Ok(client) = self.source(state.source) else {
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
            let Ok(client) = self.source(id) else {
                continue;
            };
            let todo = self.db.to_identify(id, None).await?;
            self.status.write().expect("verrou de l'état").total += todo.len();
            for series in todo {
                let names = series.names.clone();
                let client = client.clone();
                let found = blocking(move || client.search(&names[0])).await;
                let outcome = match found {
                    Ok(found) => {
                        let names: Vec<&str> = series.names.iter().map(String::as_str).collect();
                        match auto_match(&names, &found) {
                            Some(i) => Outcome::Linked {
                                external_id: found[i].id.clone(),
                                matched_by: MatchedBy::Auto,
                                data: Box::new(found[i].clone()),
                            },
                            None => {
                                Outcome::Candidates(found.into_iter().take(CANDIDATES).collect())
                            }
                        }
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
