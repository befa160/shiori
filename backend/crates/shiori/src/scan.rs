//! `shiori scan` : scan d'intégrité d'un ou plusieurs dossiers.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use comfy_table::{ContentArrangement, Table, presets};
use serde::Serialize;
use shiori_core::{FileReport, ScanOptions, Severity, Status, scan_file};
use shiori_db::{RunCounts, RunStatus, Scanned};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[derive(Args)]
pub struct ScanArgs {
    /// Dossiers (parcourus récursivement) ou fichiers .cbz.
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Décode entièrement chaque image (lent ; AVIF : en-tête seulement).
    #[arg(long)]
    deep: bool,
    /// Nombre de fichiers scannés en parallèle (défaut : nombre de cœurs).
    #[arg(short, long)]
    jobs: Option<NonZeroUsize>,
    /// Écrit le rapport complet en JSON dans ce fichier (`-` : sortie standard).
    #[arg(long, value_name = "FICHIER|-")]
    json: Option<PathBuf>,
    /// Base SQLite (défaut : <dossier de données>/shiori.db, ou $SHIORI_DATA_DIR/shiori.db).
    #[arg(long, value_name = "FICHIER", conflicts_with = "no_db")]
    db: Option<PathBuf>,
    /// N'utilise pas de base : rapport seul, tout est rescanné.
    #[arg(long)]
    no_db: bool,
    /// Rehashe et rescanne même les fichiers inchangés.
    #[arg(long)]
    rehash: bool,
    /// Gravité à partir de laquelle le code de sortie vaut 1.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,
}

#[derive(Clone, Copy, ValueEnum)]
enum FailOn {
    Error,
    Warning,
}

/// Rapport JSON. `report_version` change à chaque modification incompatible du format.
#[derive(Serialize)]
struct JsonReport<'a> {
    report_version: u32,
    options: JsonOptions,
    summary: &'a Summary,
    files: Vec<JsonFile<'a>>,
}

#[derive(Serialize)]
struct JsonOptions {
    deep: bool,
    rehash: bool,
    database: bool,
}

#[derive(Serialize)]
struct JsonFile<'a> {
    #[serde(flatten)]
    report: &'a FileReport,
    skipped: bool,
}

#[derive(Serialize, Default, Debug, PartialEq, Eq)]
struct Summary {
    files: usize,
    ok: usize,
    warning: usize,
    error: usize,
    skipped: usize,
}

/// Résultat d'un fichier. Sans `--json`, la liste des pages est vidée dès le scan
/// (seul leur nombre est affiché) : la mémoire ne grossit pas avec la bibliothèque.
struct Outcome {
    scanned: Scanned,
    page_count: usize,
}

impl Outcome {
    fn new(mut scanned: Scanned, keep_pages: bool) -> Self {
        let page_count = scanned.report.pages.len();
        if !keep_pages {
            scanned.report.pages = Vec::new();
        }
        Self {
            scanned,
            page_count,
        }
    }
}

impl Summary {
    fn of(results: &[Outcome]) -> Self {
        let mut s = Self {
            files: results.len(),
            ..Self::default()
        };
        for Outcome { scanned: r, .. } in results {
            match r.report.status {
                Status::Ok => s.ok += 1,
                Status::Warning => s.warning += 1,
                Status::Error => s.error += 1,
            }
            s.skipped += usize::from(r.skipped);
        }
        s
    }
}

pub async fn run(args: ScanArgs, quiet: bool) -> Result<ExitCode> {
    let started = Instant::now();
    let files = collect(&args.paths)?;
    let options = ScanOptions { deep: args.deep };

    let db = match args.no_db {
        true => None,
        false => Some(Arc::new(crate::open_db(args.db.clone()).await?)),
    };
    let json_options = JsonOptions {
        deep: args.deep,
        rehash: args.rehash,
        database: db.is_some(),
    };
    let run_id = match &db {
        Some(db) => {
            let roots = args
                .paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            let run = db
                .start_run(&roots, &serde_json::to_string(&json_options)?)
                .await?;
            db.set_run_total(run, files.len() as u64).await?;
            Some(run)
        }
        None => None,
    };

    let jobs = args
        .jobs
        .or_else(|| std::thread::available_parallelism().ok())
        .map_or(1, NonZeroUsize::get);
    let limit = Arc::new(Semaphore::new(jobs));
    let mut tasks = JoinSet::new();
    for (index, file) in files.iter().enumerate() {
        let (db, limit, path) = (db.clone(), limit.clone(), file.path.clone());
        let rehash = args.rehash;
        let keep_pages = args.json.is_some();
        tasks.spawn(async move {
            let _permit = limit.acquire_owned().await?;
            let scanned = match (&db, run_id) {
                (Some(db), Some(run)) => {
                    let scanned = db.scan(run, &path, options, rehash).await?;
                    db.log_file(run, &scanned).await?;
                    scanned
                }
                _ => Scanned {
                    report: tokio::task::spawn_blocking(move || scan_file(&path, options)).await?,
                    skipped: false,
                },
            };
            tracing::info!(
                file = %scanned.report.path.display(),
                status = ?scanned.report.status,
                skipped = scanned.skipped,
                "scanné"
            );
            anyhow::Ok((index, Outcome::new(scanned, keep_pages)))
        });
    }
    let mut results: Vec<Option<Outcome>> = files.iter().map(|_| None).collect();
    let mut failure = None;
    while let Some(done) = tasks.join_next().await {
        match done.map_err(anyhow::Error::from).and_then(|r| r) {
            Ok((index, scanned)) => results[index] = Some(scanned),
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    let results: Vec<Outcome> = results.into_iter().flatten().collect();
    if let (Some(db), Some(run)) = (&db, run_id) {
        let mut counts = RunCounts::default();
        results.iter().for_each(|r| counts.add(&r.scanned));
        // Une erreur fatale clôt la passe `failed` : jamais laissée `running`.
        let (status, error) = match &failure {
            Some(e) => (RunStatus::Failed, Some(format!("{e:#}"))),
            None => (RunStatus::Completed, None),
        };
        db.finish_run(run, status, &counts, error.as_deref())
            .await?;
    }
    if let Some(e) = failure {
        return Err(e);
    }

    let summary = Summary::of(&results);
    let json_to_stdout = args.json.as_deref() == Some(Path::new("-"));
    if let Some(target) = &args.json {
        let report = JsonReport {
            report_version: 1,
            options: json_options,
            summary: &summary,
            files: results
                .iter()
                .map(|r| JsonFile {
                    report: &r.scanned.report,
                    skipped: r.scanned.skipped,
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&report)?;
        if json_to_stdout {
            println!("{text}");
        } else {
            fs::write(target, text + "\n")
                .with_context(|| format!("écriture de {}", target.display()))?;
        }
    }

    if !quiet {
        let text = format!(
            "{}\n{}\n",
            table(&files, &results),
            summary_line(&summary, started.elapsed().as_secs_f64())
        );
        // Le JSON sur la sortie standard doit rester exploitable tel quel.
        if json_to_stdout {
            std::io::stderr().write_all(text.as_bytes())?;
        } else {
            std::io::stdout().write_all(text.as_bytes())?;
        }
    }
    Ok(exit_code(&results, args.fail_on))
}

/// Fichier à scanner et nom affiché (relatif au dossier passé en argument).
struct Target {
    path: PathBuf,
    shown: String,
}

/// Fichiers à scanner, triés et sans doublon. Les liens symboliques rencontrés dans les
/// dossiers ne sont pas suivis ; un fichier passé explicitement est toujours scanné.
fn collect(roots: &[PathBuf]) -> Result<Vec<Target>> {
    let mut found = BTreeMap::new();
    for root in roots {
        let meta = fs::metadata(root).with_context(|| format!("{} introuvable", root.display()))?;
        if meta.is_file() {
            found.insert(root.clone(), root.display().to_string());
            continue;
        }
        let mut files = Vec::new();
        walk(root, &mut files);
        for file in files {
            let shown = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .display()
                .to_string();
            found.entry(file).or_insert(shown);
        }
    }
    if found.is_empty() {
        bail!("aucun fichier .cbz trouvé");
    }
    Ok(found
        .into_iter()
        .map(|(path, shown)| Target { path, shown })
        .collect())
}

pub(crate) fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("dossier illisible, ignoré : {} ({e})", dir.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            walk(&path, out);
        } else if kind.is_file() && is_cbz(&path) {
            out.push(path);
        }
    }
}

fn is_cbz(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cbz"))
}

fn exit_code(results: &[Outcome], fail_on: FailOn) -> ExitCode {
    let failed = results.iter().any(|r| {
        matches!(
            (fail_on, r.scanned.report.status),
            (_, Status::Error) | (FailOn::Warning, Status::Warning)
        )
    });
    ExitCode::from(u8::from(failed))
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warning => "avertissement",
        Status::Error => "erreur",
    }
}

/// Codes des problèmes (hors simples informations), avec leur nombre.
fn diagnostics(report: &FileReport) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for issue in report.issues.iter().filter(|i| i.severity > Severity::Info) {
        let code = serde_json::to_value(issue.code)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        *counts.entry(code).or_default() += 1;
    }
    if counts.is_empty() {
        return "—".into();
    }
    counts
        .into_iter()
        .map(|(code, n)| if n > 1 { format!("{code} ×{n}") } else { code })
        .collect::<Vec<_>>()
        .join(", ")
}

fn table(files: &[Target], results: &[Outcome]) -> Table {
    let mut table = Table::new();
    table
        .load_style(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(["Fichier", "Statut", "Pages", "Diagnostics"]);
    for (
        file,
        Outcome {
            scanned: r,
            page_count,
        },
    ) in files.iter().zip(results)
    {
        let status = status_label(r.report.status);
        let status = if r.skipped {
            format!("{status} (inchangé)")
        } else {
            status.to_owned()
        };
        table.add_row([
            file.shown.clone(),
            status,
            page_count.to_string(),
            diagnostics(&r.report),
        ]);
    }
    table
}

fn summary_line(s: &Summary, seconds: f64) -> String {
    format!(
        "{} fichier(s) : {} ok, {} avertissement(s), {} erreur(s) ; {} inchangé(s) non rescanné(s) ; {seconds:.1} s",
        s.files, s.ok, s.warning, s.error, s.skipped
    )
}

#[cfg(test)]
mod tests;
