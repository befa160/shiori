//! Job de scan du serveur : une passe à la fois, sur une bibliothèque, un dossier ou un
//! fichier situés sous sa racine. Même moteur que la CLI (`Db::scan`), puis rattachement aux
//! séries ; progression en mémoire, annulation entre deux fichiers.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use shiori_core::ScanOptions;
use shiori_db::{Db, Library, RunCounts, RunOrigin, RunStatus};
use tokio::sync::{Mutex, RwLock};
use tokio::task::{JoinHandle, JoinSet};

/// Ce qu'il faut scanner : toute la bibliothèque, ou un dossier / un fichier sous sa racine.
#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub library_id: i64,
    pub path: Option<PathBuf>,
    pub deep: bool,
    pub rehash: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("un scan est déjà en cours")]
    AlreadyRunning,
    #[error("bibliothèque inconnue")]
    UnknownLibrary,
    #[error("{0} n'est pas sous la racine de la bibliothèque")]
    OutsideLibrary(String),
    #[error("{0} introuvable")]
    NotFound(String),
    #[error(transparent)]
    Db(#[from] shiori_db::Error),
}

/// État d'une passe en cours, tel que l'API le montre.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub run: i64,
    pub library_id: i64,
    pub target: String,
    pub total: u64,
    pub done: u64,
    /// Dernier fichier commencé.
    pub current: Option<String>,
    pub counts: RunCounts,
    pub cancelling: bool,
}

struct Running {
    progress: Arc<RwLock<Progress>>,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct Scanner {
    jobs: usize,
    log_days: u32,
    running: Mutex<Option<Running>>,
    /// Identification des nouvelles séries à la fin d'une passe complète.
    metadata: Option<Arc<crate::meta::Metadata>>,
}

enum Target {
    /// Bibliothèque entière ou dossier : les fichiers disparus en sont retirés à la fin.
    Folder(PathBuf),
    File(PathBuf),
}

impl Scanner {
    /// `jobs` fichiers scannés en parallèle ; passes gardées `log_days` jours.
    pub fn new(jobs: usize, log_days: u32) -> Self {
        Self {
            jobs: jobs.max(1),
            log_days,
            running: Mutex::new(None),
            metadata: None,
        }
    }

    /// À la fin de chaque passe terminée, les séries pas encore identifiées le sont (si le
    /// réseau est activé).
    pub fn with_metadata(self, metadata: Arc<crate::meta::Metadata>) -> Self {
        Self {
            metadata: Some(metadata),
            ..self
        }
    }

    /// Lance une passe et renvoie son numéro ; refuse s'il en tourne déjà une.
    pub async fn start(&self, db: Arc<Db>, request: ScanRequest) -> Result<i64, StartError> {
        let mut running = self.running.lock().await;
        if running.as_ref().is_some_and(|r| !r.handle.is_finished()) {
            return Err(StartError::AlreadyRunning);
        }
        let library = db
            .libraries()
            .await?
            .into_iter()
            .find(|l| l.id == request.library_id)
            .ok_or(StartError::UnknownLibrary)?;
        let target = target(&library, request.path.as_deref())?;
        let shown = match &target {
            Target::Folder(p) | Target::File(p) => p.display().to_string(),
        };
        let options = serde_json::json!({ "deep": request.deep, "rehash": request.rehash });
        let run = db
            .begin_run(
                RunOrigin::Api,
                Some(library.id),
                &shown,
                &options.to_string(),
            )
            .await?;
        let progress = Arc::new(RwLock::new(Progress {
            run,
            library_id: library.id,
            target: shown,
            total: 0,
            done: 0,
            current: None,
            counts: RunCounts::default(),
            cancelling: false,
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let pass = Pass {
            db,
            library,
            run,
            options: ScanOptions { deep: request.deep },
            rehash: request.rehash,
            jobs: self.jobs,
            progress: progress.clone(),
            cancel: cancel.clone(),
            metadata: self.metadata.clone(),
        };
        let log_days = self.log_days;
        let handle = tokio::spawn(async move { pass.run(target, log_days).await });
        *running = Some(Running {
            progress,
            cancel,
            handle,
        });
        Ok(run)
    }

    /// Progression de la passe en cours, ou `None` si rien ne tourne.
    pub async fn progress(&self) -> Option<Progress> {
        let running = self.running.lock().await;
        let running = running.as_ref().filter(|r| !r.handle.is_finished())?;
        Some(running.progress.read().await.clone())
    }

    /// Demande l'arrêt de la passe en cours après les fichiers déjà commencés. `false` si
    /// rien ne tourne.
    pub async fn cancel(&self) -> bool {
        let running = self.running.lock().await;
        let Some(running) = running.as_ref().filter(|r| !r.handle.is_finished()) else {
            return false;
        };
        running.cancel.store(true, Ordering::Relaxed);
        running.progress.write().await.cancelling = true;
        true
    }

    /// Attend la fin de la passe en cours (rien si aucune).
    pub async fn wait(&self) {
        if let Some(running) = self.running.lock().await.take() {
            let _ = running.handle.await;
        }
    }

    /// Arrêt du serveur : annule la passe en cours et attend qu'elle soit close.
    pub async fn shutdown(&self) {
        self.cancel().await;
        self.wait().await;
    }
}

/// Valide la cible : la racine elle-même, ou un chemin dessous, sans `..` ni lien
/// symbolique (l'API ne doit pas permettre de lire n'importe quel dossier du serveur).
fn target(library: &Library, path: Option<&Path>) -> Result<Target, StartError> {
    let root = PathBuf::from(&library.root);
    let Some(path) = path else {
        return Ok(Target::Folder(root));
    };
    let shown = path.display().to_string();
    let escapes = path.components().any(|c| c == Component::ParentDir);
    if escapes || !path.starts_with(&root) {
        return Err(StartError::OutsideLibrary(shown));
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| StartError::NotFound(shown.clone()))?;
    if meta.is_symlink() {
        return Err(StartError::OutsideLibrary(shown));
    }
    Ok(match meta.is_dir() {
        true => Target::Folder(path.to_path_buf()),
        false => Target::File(path.to_path_buf()),
    })
}

struct Pass {
    db: Arc<Db>,
    library: Library,
    run: i64,
    options: ScanOptions,
    rehash: bool,
    jobs: usize,
    progress: Arc<RwLock<Progress>>,
    cancel: Arc<AtomicBool>,
    metadata: Option<Arc<crate::meta::Metadata>>,
}

impl Pass {
    async fn run(self, target: Target, log_days: u32) {
        let (status, error) = match self.scan(target).await {
            Ok(true) => (RunStatus::Completed, None),
            Ok(false) => (RunStatus::Cancelled, None),
            Err(e) => {
                tracing::error!("scan {} : {e}", self.run);
                (RunStatus::Failed, Some(e.to_string()))
            }
        };
        let counts = self.progress.read().await.counts;
        let closed = self
            .db
            .finish_run(self.run, status, &counts, error.as_deref());
        if let Err(e) = closed.await {
            tracing::error!("clôture du scan {} : {e}", self.run);
        }
        if let Err(e) = self.db.purge_runs(log_days).await {
            tracing::warn!("purge des anciens scans : {e}");
        }
        // Déjà en cours : les nouvelles séries attendront la prochaine identification.
        if status == RunStatus::Completed
            && let Some(metadata) = &self.metadata
        {
            let _ = metadata.identify();
        }
    }

    /// `Ok(false)` si la passe a été annulée avant d'avoir vu tous les fichiers.
    async fn scan(&self, target: Target) -> Result<bool, shiori_db::Error> {
        let (files, folder) = match target {
            Target::Folder(dir) => {
                let mut files = Vec::new();
                crate::scan::walk(&dir, &mut files);
                files.sort();
                (files, Some(dir))
            }
            Target::File(file) => (vec![file], None),
        };
        self.db.set_run_total(self.run, files.len() as u64).await?;
        self.progress.write().await.total = files.len() as u64;

        let mut tasks = JoinSet::new();
        let mut stopped = false;
        for path in files {
            if self.cancel.load(Ordering::Relaxed) {
                stopped = true;
                break;
            }
            while tasks.len() >= self.jobs {
                self.finished(tasks.join_next().await).await?;
            }
            self.progress.write().await.current = Some(path.display().to_string());
            let (db, library, run) = (self.db.clone(), self.library.clone(), self.run);
            let (options, rehash) = (self.options, self.rehash);
            tasks.spawn(async move {
                let mut scanned = db.scan(run, &path, options, rehash).await?;
                db.attach(&library, &path).await?;
                db.log_file(run, &scanned).await?;
                scanned.report.pages = Vec::new(); // seuls les compteurs servent ensuite
                Ok::<_, shiori_db::Error>(scanned)
            });
        }
        while !tasks.is_empty() {
            self.finished(tasks.join_next().await).await?;
        }
        if stopped {
            return Ok(false);
        }
        if let Some(dir) = folder {
            let removed = self
                .db
                .remove_missing(&self.library, &dir, self.run)
                .await?;
            self.progress.write().await.counts.removed = removed;
        }
        self.db.refresh_series(&self.library).await?;
        Ok(true)
    }

    async fn finished(
        &self,
        done: Option<Result<Result<shiori_db::Scanned, shiori_db::Error>, tokio::task::JoinError>>,
    ) -> Result<(), shiori_db::Error> {
        let Some(done) = done else {
            return Ok(());
        };
        let scanned = done.map_err(|e| shiori_db::Error::Io(std::io::Error::other(e)))??;
        let mut progress = self.progress.write().await;
        progress.counts.add(&scanned);
        progress.done += 1;
        Ok(())
    }
}
