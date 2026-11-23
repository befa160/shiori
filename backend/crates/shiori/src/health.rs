//! `shiori health` : demande `/api/health` à un Shiori qui tourne, pour le `HEALTHCHECK` de
//! l'image Docker (distroless : ni shell, ni curl). Code 0 si la réponse est 200, 1 sinon, même
//! sans serveur.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Une réponse plus lente signale un serveur bloqué : Docker le compte comme un échec.
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Args)]
pub struct HealthArgs {
    /// Adresse du Shiori à interroger (celle de `serve --listen`, vue depuis cette machine).
    #[arg(long, default_value = "127.0.0.1:7878", env = "SHIORI_HEALTH_ADDR")]
    pub addr: SocketAddr,
}

/// Toujours 0 ou 1, jamais 2 : Docker réserve ce code, `HEALTHCHECK` n'attend que 0 ou 1.
pub async fn run(args: HealthArgs) -> ExitCode {
    let checked = tokio::time::timeout(TIMEOUT, status(args.addr))
        .await
        .context("pas de réponse dans les 5 s")
        .and_then(|status| status);
    match checked {
        Ok(200) => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("/api/health a répondu {status}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("Shiori ne répond pas : {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Statut HTTP de `GET /api/health`. `Host: localhost` : toujours accepté par la parade au DNS
/// rebinding, quelle que soit l'adresse d'écoute.
async fn status(addr: SocketAddr) -> Result<u16> {
    let mut stream = TcpStream::connect(addr)
        .await
        .with_context(|| format!("connexion à {addr}"))?;
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await?;
    // La ligne de statut tient dans les premiers octets : `HTTP/1.1 200 OK`.
    let mut head = [0; 12];
    stream.read_exact(&mut head).await?;
    std::str::from_utf8(&head[9..12])
        .ok()
        .and_then(|code| code.parse().ok())
        .context("réponse qui n'est pas du HTTP")
}

#[cfg(test)]
mod tests;
