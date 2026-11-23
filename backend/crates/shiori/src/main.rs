//! Binaire Shiori : CLI de scan d'intégrité et serveur HTTP.

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use shiori::{backup, health, scan, server, user};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "shiori",
    version,
    about = "Lecteur manga / BD / webtoon auto-hébergé"
)]
struct Cli {
    /// Plus de journaux sur la sortie d'erreur (-v : info, -vv : debug).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,
    /// Silencieux : `scan` n'affiche ni tableau ni résumé, `serve` ne dit pas où il écoute.
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan d'intégrité des archives d'un ou plusieurs dossiers.
    Scan(scan::ScanArgs),
    /// Serveur HTTP : API JSON des bibliothèques, séries, tomes et pages.
    Serve(server::ServeArgs),
    /// Comptes : création, mot de passe oublié (base locale, sans passer par le serveur).
    User(user::UserArgs),
    /// Sauvegarde de la base, cohérente même Shiori allumé, dans le dossier des sauvegardes.
    Backup(backup::BackupArgs),
    /// Vérifie qu'un Shiori lancé répond (`HEALTHCHECK` de Docker) : code 0 s'il répond, 1 sinon.
    Health(health::HealthArgs),
}

fn main() -> ExitCode {
    // Double-clic sur le binaire : aucun argument, on lance le serveur et ouvre l'interface au
    // lieu de l'aide, dont la fenêtre se refermerait aussitôt.
    let double_click = std::env::args_os().len() == 1;
    let cli = match double_click {
        true => Cli::parse_from(["shiori", "serve", "--open"]),
        false => Cli::parse(),
    };
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()))
        .init();

    let outcome = tokio::runtime::Runtime::new()
        .map_err(anyhow::Error::from)
        .and_then(|rt| {
            rt.block_on(async {
                match cli.command {
                    Command::Scan(args) => scan::run(args, cli.quiet).await,
                    Command::Serve(args) => {
                        server::run(args, cli.quiet, stop_signal()).await?;
                        Ok(ExitCode::SUCCESS)
                    }
                    Command::User(args) => {
                        user::run(args).await?;
                        Ok(ExitCode::SUCCESS)
                    }
                    Command::Backup(args) => {
                        backup::run(args).await?;
                        Ok(ExitCode::SUCCESS)
                    }
                    Command::Health(args) => Ok(health::run(args).await),
                }
            })
        });
    match outcome {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erreur : {e:#}");
            // Sans terminal ouvert, l'erreur doit rester lisible avant que la fenêtre se ferme.
            if double_click {
                eprintln!("Appuyez sur Entrée pour fermer.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            ExitCode::from(2)
        }
    }
}

/// Ctrl-C, et SIGTERM sous Unix (`docker stop`, systemd) : sans lui, le conteneur est tué au
/// bout de 10 s, scan en cours compris.
async fn stop_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            },
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
