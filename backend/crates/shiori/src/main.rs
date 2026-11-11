//! Binaire Shiori : CLI de scan d'intégrité, puis le serveur web (phase 2).

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod scan;

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
    /// N'affiche ni tableau ni résumé : seul le code de sortie compte.
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan d'intégrité des archives d'un ou plusieurs dossiers.
    Scan(scan::ScanArgs),
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

    let Command::Scan(args) = cli.command;
    let outcome = tokio::runtime::Runtime::new()
        .map_err(anyhow::Error::from)
        .and_then(|rt| rt.block_on(scan::run(args, cli.quiet)));
    match outcome {
        Ok(code) => code,
        Err(e) => {
            eprintln!("erreur : {e:#}");
            ExitCode::from(2)
        }
    }
}
