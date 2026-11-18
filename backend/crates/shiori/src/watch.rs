//! Surveillance des dossiers (lot 9.1) : un changement sous une bibliothèque surveillée lance,
//! après un temps calme (fichier encore en copie), un scan ciblé sur le dossier de série
//! touché. Un scan complet périodique de toutes les bibliothèques sert de filet (montages
//! réseau dont les changements faits ailleurs ne sont pas signalés, serveur éteint).
//!
//! Pas de file dans le `Scanner` : les cibles attendent ici, une par dossier, et partent une à
//! une quand aucune passe ne tourne.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::time::Instant;

use crate::job::{ScanRequest, StartError, Trigger};
use crate::server::App;

/// Délais de la surveillance ; plus courts dans les tests.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Sans nouvel événement pendant ce temps, le dossier est scanné.
    pub calm: Duration,
    /// Fréquence des vérifications (cibles mûres, bibliothèques ajoutées ou retirées).
    pub tick: Duration,
    /// Délai avant de retenter une racine impossible à surveiller.
    pub retry: Duration,
    /// Unité du réglage `full_scan_hours`.
    pub hour: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            calm: Duration::from_secs(30),
            tick: Duration::from_secs(5),
            retry: Duration::from_secs(300),
            hour: Duration::from_secs(3600),
        }
    }
}

/// Ce qui mérite un scan. Lire un fichier (le scan lui-même, le lecteur) produit des accès
/// et parfois une date d'accès : les compter relancerait un scan sans fin.
fn relevant(kind: &EventKind, path: &Path) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(ModifyKind::Name(_)) => true,
        // Windows signale aussi le dossier dont une entrée change : l'entrée a déjà son
        // événement, et le dossier d'une série viserait toute la racine.
        EventKind::Modify(_) => !path.is_dir(),
        _ => true,
    }
}

/// Dossier à scanner pour un changement en `path` : le dossier de série (premier niveau
/// sous la racine) ; la racine elle-même pour ce qui est posé, renommé ou retiré directement
/// dessous (un renommage de série n'est reconnu que si une même passe voit les deux noms).
/// `real` : la racine sans lien symbolique, telle que macOS la signale (`/private/var/…` pour
/// `/var/…`) ; le dossier rendu est toujours sous la racine enregistrée.
fn target(roots: &[(i64, PathBuf, PathBuf)], path: &Path) -> Option<(i64, PathBuf)> {
    roots.iter().find_map(|(id, root, real)| {
        let rest = path.strip_prefix(root).or_else(|_| path.strip_prefix(real));
        let mut rest = rest.ok()?.components();
        let folder = match (rest.next(), rest.next()) {
            (Some(first), Some(_)) => root.join(first),
            _ => root.clone(),
        };
        Some((*id, folder))
    })
}

struct Pending {
    due: Instant,
    trigger: Trigger,
}

/// Lance la surveillance pour toute la vie du serveur.
pub fn spawn(app: Arc<App>, timing: Timing) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(app, timing))
}

async fn run(app: Arc<App>, timing: Timing) {
    let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })
    .inspect_err(|e| tracing::error!("surveillance des dossiers indisponible : {e}"))
    .ok();
    // Racines surveillées, et celles en échec avec la date du prochain essai.
    let mut watched: Vec<(i64, PathBuf, PathBuf)> = Vec::new();
    let mut failed: HashMap<i64, Instant> = HashMap::new();
    let mut pending: HashMap<(i64, PathBuf), Pending> = HashMap::new();
    let mut last_full = Instant::now();
    // Bibliothèques vues au tick précédent ; `None` avant le premier (démarrage).
    let mut seen: Option<HashSet<i64>> = None;
    // Ce que Shiori écrit lui-même : base rangée dans une bibliothèque, cache, sauvegardes.
    // Sans ça, chaque scan (qui écrit la base) en relancerait un autre.
    let db_file = app.db.pool().connect_options().get_filename().to_path_buf();
    let db_file = std::path::absolute(&db_file).unwrap_or(db_file);
    let own = [app.images.dir().to_path_buf(), app.backups.dir.clone()];
    let ours = move |path: &Path| {
        own.iter().any(|dir| path.starts_with(dir))
            || path.parent() == db_file.parent()
                && path
                    .file_name()
                    .zip(db_file.file_name())
                    .is_some_and(|(name, db)| {
                        name.as_encoded_bytes().starts_with(db.as_encoded_bytes())
                    })
    };
    let mut tick = tokio::time::interval(timing.tick);
    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                let event: Event = match event {
                    Ok(event) => event,
                    Err(e) => {
                        tracing::warn!("surveillance des dossiers : {e}");
                        continue;
                    }
                };
                // File d'événements débordée : tout peut avoir changé.
                let paths: Vec<PathBuf> = match event.need_rescan() {
                    true => watched.iter().map(|(_, root, _)| root.clone()).collect(),
                    false => event
                        .paths
                        .into_iter()
                        .filter(|p| relevant(&event.kind, p) && !ours(p))
                        .collect(),
                };
                for path in paths {
                    let Some(key) = target(&watched, &path) else { continue };
                    // Racine retirée ou démontée : le système a lâché sa surveillance.
                    tracing::debug!("{:?} {} : scan de {}", event.kind, path.display(), key.1.display());
                    let removed = matches!(event.kind, EventKind::Remove(_));
                    watched.retain(|(id, root, real)| {
                        !(removed && *id == key.0 && (path == *root || path == *real))
                    });
                    let due = Instant::now() + timing.calm;
                    pending.insert(key, Pending { due, trigger: Trigger::Watch });
                }
            }
            _ = tick.tick() => {
                let Ok(libraries) = app.db.libraries().await else { continue };
                if let Some(watcher) = watcher.as_mut() {
                    for library in &libraries {
                        let root = PathBuf::from(&library.root);
                        let known = watched.iter().any(|(id, _, _)| *id == library.id);
                        let waiting = failed.get(&library.id).is_some_and(|t| *t > Instant::now());
                        if !library.watch || known || waiting {
                            continue;
                        }
                        match watcher.watch(&root, RecursiveMode::Recursive) {
                            Ok(()) => {
                                if failed.remove(&library.id).is_some() {
                                    tracing::info!("{} de nouveau surveillé", root.display());
                                }
                                let real = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
                                watched.push((library.id, root.clone(), real));
                                // Ce qui a changé pendant qu'on ne regardait pas. Pas pour une
                                // bibliothèque qui vient d'être créée : l'interface lance son
                                // propre scan, qui sinon recevrait `scan_already_running`.
                                if seen.as_ref().is_none_or(|ids| ids.contains(&library.id)) {
                                    let due = Instant::now();
                                    pending.insert((library.id, root), Pending { due, trigger: Trigger::Watch });
                                }
                            }
                            Err(e) => {
                                // Trop de dossiers : sous Linux, fs.inotify.max_user_watches.
                                tracing::warn!("{} ne peut pas être surveillé : {e}", root.display());
                                failed.insert(library.id, Instant::now() + timing.retry);
                            }
                        }
                    }
                    watched.retain(|(id, root, _)| {
                        let keep = libraries.iter().any(|l| l.id == *id && l.watch);
                        if !keep {
                            let _ = watcher.unwatch(root);
                        }
                        keep
                    });
                }
                seen = Some(libraries.iter().map(|l| l.id).collect());
                let hours = app.db.settings().await.map_or(0, |s| s.full_scan_hours);
                if hours > 0 && last_full.elapsed() >= timing.hour * hours {
                    last_full = Instant::now();
                    for library in &libraries {
                        let key = (library.id, PathBuf::from(&library.root));
                        pending.insert(key, Pending { due: last_full, trigger: Trigger::Schedule });
                    }
                }
                start_next(&app, &libraries, &mut pending).await;
            }
        }
    }
}

/// Lance la cible mûre la plus ancienne si aucune passe ne tourne.
async fn start_next(
    app: &App,
    libraries: &[shiori_db::Library],
    pending: &mut HashMap<(i64, PathBuf), Pending>,
) {
    let now = Instant::now();
    let Some(key) = pending
        .iter()
        .filter(|(_, p)| p.due <= now)
        .min_by_key(|(_, p)| p.due)
        .map(|(key, _)| key.clone())
    else {
        return;
    };
    let (library_id, folder) = key.clone();
    let root = libraries.iter().find(|l| l.id == library_id);
    let Some(root) = root.map(|l| PathBuf::from(&l.root)) else {
        pending.remove(&key); // bibliothèque supprimée
        return;
    };
    // Dossier de série supprimé : la racine, pour retirer ses tomes.
    let folder = match folder.exists() {
        true => folder,
        false => root.clone(),
    };
    let request = ScanRequest {
        library_id,
        path: (folder != root).then(|| folder.clone()),
        deep: false,
        rehash: false,
        trigger: pending[&key].trigger,
    };
    match app.scanner.start(app.db.clone(), request).await {
        Ok(run) => {
            tracing::info!("scan {run} de {} (surveillance)", folder.display());
            pending.remove(&key);
            // La racine couvre tous les dossiers de la bibliothèque en attente.
            if folder == root {
                pending.retain(|(id, _), _| *id != library_id);
            }
        }
        Err(StartError::AlreadyRunning | StartError::ShuttingDown) => {}
        Err(e) => {
            tracing::warn!("scan de {} (surveillance) : {e}", folder.display());
            pending.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests;
