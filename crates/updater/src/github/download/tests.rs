use std::time::Duration;

use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
    time::Instant,
};

use crate::api::UpdateErrorKind;

use super::{build_client, download_archive_from};

#[tokio::test]
async fn continuous_progress_can_outlive_one_read_timeout_window() {
    let chunks = (0_u8..8).map(|value| vec![value; 8]).collect::<Vec<_>>();
    let expected = chunks.concat();
    let (url, server) = streaming_server(chunks, Duration::from_millis(40)).await;
    let client = build_client(false, Duration::from_millis(150)).expect("test client");
    let directory = tempdir().expect("temporary directory");
    let archive = directory.path().join("release.tar.gz");
    let mut progress = Vec::new();
    let started = Instant::now();

    let digest = download_archive_from(&client, &url, expected.len() as u64, &archive, |bytes| {
        progress.push(bytes)
    })
    .await
    .expect("continuous slow download");
    server.await.expect("streaming server");

    assert!(started.elapsed() > Duration::from_millis(250));
    assert_eq!(tokio::fs::read(archive).await.expect("archive"), expected);
    assert_eq!(digest, format!("{:x}", Sha256::digest(&expected)));
    assert_eq!(progress.last(), Some(&(expected.len() as u64)));
}

#[tokio::test]
async fn a_stalled_body_fails_at_the_read_progress_timeout() {
    let chunks = vec![b"first".to_vec(), b"second".to_vec()];
    let expected_size = chunks.iter().map(Vec::len).sum::<usize>() as u64;
    let (url, server) = streaming_server(chunks, Duration::from_millis(400)).await;
    let client = build_client(false, Duration::from_millis(75)).expect("test client");
    let directory = tempdir().expect("temporary directory");
    let archive = directory.path().join("release.tar.gz");

    let error = download_archive_from(&client, &url, expected_size, &archive, |_| {})
        .await
        .expect_err("stalled download must fail");
    server.abort();
    let _ = server.await;

    assert_eq!(error.kind(), UpdateErrorKind::DownloadFailed);
}

async fn streaming_server(chunks: Vec<Vec<u8>>, pause: Duration) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test listener");
    let address = listener.local_addr().expect("listener address");
    let content_length = chunks.iter().map(Vec::len).sum::<usize>();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("test connection");
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request).await;
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
        );
        if stream.write_all(headers.as_bytes()).await.is_err() {
            return;
        }
        for (index, chunk) in chunks.into_iter().enumerate() {
            if index > 0 {
                tokio::time::sleep(pause).await;
            }
            if stream.write_all(&chunk).await.is_err() {
                return;
            }
            let _ = stream.flush().await;
        }
    });
    (format!("http://{address}/release.tar.gz"), server)
}
