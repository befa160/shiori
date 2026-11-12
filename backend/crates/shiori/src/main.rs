//! Binaire Shiori : CLI de scan d'intégrité et serveur HTTP.

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use shiori::{scan, server};
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
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
                        let ctrl_c = async {
                            let _ = tokio::signal::ctrl_c().await;
                        };
                        server::run(args, cli.quiet, ctrl_c).await?;
                        Ok(ExitCode::SUCCESS)
                    }
                }
            })
        });
    match outcome {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erreur : {e:#}");
            ExitCode::from(2)
        }
    }
}
