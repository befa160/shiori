use std::process::ExitCode;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{HealthArgs, run};

/// Faux serveur : lit la requête, répond `response`, et rend la requête reçue.
async fn answering(response: &'static [u8]) -> (HealthArgs, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let served = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0; 1024];
        let n = socket.read(&mut request).await.unwrap();
        socket.write_all(response).await.unwrap();
        String::from_utf8_lossy(&request[..n]).into_owned()
    });
    (HealthArgs { addr }, served)
}

#[tokio::test]
async fn a_200_is_healthy_and_asks_localhost() {
    let (args, served) = answering(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await;
    assert_eq!(run(args).await, ExitCode::SUCCESS);
    let request = served.await.unwrap();
    assert!(
        request.starts_with("GET /api/health HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(request.contains("Host: localhost\r\n"), "{request}");
}

#[tokio::test]
async fn any_other_status_is_unhealthy() {
    let (args, _served) = answering(b"HTTP/1.1 503 Service Unavailable\r\n\r\n").await;
    assert_eq!(run(args).await, ExitCode::FAILURE);
}

#[tokio::test]
async fn garbage_or_no_server_is_unhealthy() {
    for response in [&b"pas du tout du HTTP"[..], b"HTTP/1.1"] {
        let (args, _served) = answering(response).await;
        assert_eq!(run(args).await, ExitCode::FAILURE, "{response:?}");
    }
    // Port libéré juste avant : personne n'écoute.
    let addr = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    assert_eq!(run(HealthArgs { addr }).await, ExitCode::FAILURE);
}

#[tokio::test]
async fn a_server_that_never_answers_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Accepte la connexion et la garde ouverte sans rien répondre.
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
        drop(socket);
    });
    let started = std::time::Instant::now();
    assert_eq!(run(HealthArgs { addr }).await, ExitCode::FAILURE);
    assert!(started.elapsed() >= super::TIMEOUT);
}
