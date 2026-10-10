#[allow(dead_code)]
mod support;
use exoharness_artifacts::{RemoteFile, same_service_origin};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn server(
    status: &str,
    extra: &str,
    body: &[u8],
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    let body = body.to_vec();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0u8; 8192];
        let n = stream.read(&mut bytes).await.unwrap();
        stream.write_all(reply.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
        String::from_utf8_lossy(&bytes[..n]).into_owned()
    });
    (origin, task)
}

#[test]
fn origin_restriction_rejects_other_ports_schemes_credentials_and_redirect_targets() {
    let origin = "http://127.0.0.1:8767/mcp";
    assert!(same_service_origin(
        origin,
        "http://127.0.0.1:8767/files/song.mp3?key=private"
    ));
    for url in [
        "http://127.0.0.1:8787/admin",
        "http://169.254.169.254/metadata",
        "https://127.0.0.1:8767/x",
        "file:///etc/passwd",
        "http://user:secret@127.0.0.1:8767/x",
        "http://127.0.0.1:8767/x#fragment",
    ] {
        assert!(!same_service_origin(origin, url));
    }
}

#[tokio::test]
async fn completed_audio_is_persisted_without_forwarding_credentials_or_changing_source() {
    let (root, _, artifacts) = tokio::task::spawn_blocking(support::fixture).await.unwrap();
    let service = Arc::new(artifacts);
    let (origin, request) = server(
        "200 OK",
        "Content-Type: audio/mpeg\r\n",
        b"existing mp3 bytes",
    )
    .await;
    let source = format!("{origin}/files/song.mp3?key=per-file-token");
    let input = RemoteFile {
        service_url: &origin,
        url: &source,
        filename: "song.mp3",
        size: Some(18),
        sha256: None,
    };
    let saved = service
        .import_remote("owner", "session", "run", input)
        .await
        .unwrap();
    assert!(saved.url.starts_with("http://127.0.0.1:9790/artifacts/"));
    assert!(!saved.url.contains("per-file-token"));
    assert_eq!(saved.artifact.content_type, "audio/mpeg");
    assert_eq!(saved.artifact.size_bytes, 18);
    let request = request.await.unwrap().to_ascii_lowercase();
    assert!(request.starts_with("get /files/song.mp3?key=per-file-token"));
    assert!(!request.contains("authorization:") && !request.contains("cookie:"));
    assert_eq!(
        std::fs::read_dir(root.path().join("files/objects"))
            .unwrap()
            .count(),
        1
    );
}

#[tokio::test]
async fn wrong_owner_is_rejected_before_any_network_request() {
    let (root, _, artifacts) = tokio::task::spawn_blocking(support::fixture).await.unwrap();
    let service = Arc::new(artifacts);
    let (origin, request) = server("200 OK", "", b"bytes").await;
    let url = format!("{origin}/song.mp3");
    assert!(
        service
            .import_remote(
                "other",
                "session",
                "run",
                RemoteFile {
                    service_url: &origin,
                    url: &url,
                    filename: "song.mp3",
                    size: None,
                    sha256: None
                }
            )
            .await
            .is_err()
    );
    assert!(!request.is_finished());
    request.abort();
    assert_eq!(
        std::fs::read_dir(root.path().join("files/objects"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn bad_response_redirect_empty_hash_size_and_wrong_mime_never_publish() {
    let (root, _, artifacts) = tokio::task::spawn_blocking(support::fixture).await.unwrap();
    let service = Arc::new(artifacts);
    for (status, headers, bytes, size, hash) in [
        (
            "302 Found",
            "Location: http://169.254.169.254/metadata\r\n",
            b"bytes".as_slice(),
            None,
            None,
        ),
        (
            "200 OK",
            "Content-Type: text/html\r\n",
            b"error".as_slice(),
            None,
            None,
        ),
        (
            "200 OK",
            "Content-Type: audio/mpeg\r\n",
            b"".as_slice(),
            None,
            None,
        ),
        (
            "200 OK",
            "Content-Type: audio/mpeg\r\n",
            b"bytes".as_slice(),
            Some(100),
            None,
        ),
        (
            "200 OK",
            "Content-Type: audio/mpeg\r\n",
            b"bytes".as_slice(),
            None,
            Some("incorrect"),
        ),
    ] {
        let (origin, request) = server(status, headers, bytes).await;
        let url = format!("{origin}/song.mp3");
        assert!(
            service
                .import_remote(
                    "owner",
                    "session",
                    "run",
                    RemoteFile {
                        service_url: &origin,
                        url: &url,
                        filename: "song.mp3",
                        size,
                        sha256: hash
                    }
                )
                .await
                .is_err()
        );
        request.await.unwrap();
    }
    assert_eq!(
        std::fs::read_dir(root.path().join("files/objects"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn cancellation_drops_an_incomplete_download_without_publishing_bytes() {
    let (root, _, artifacts) = tokio::task::spawn_blocking(support::fixture).await.unwrap();
    let service = Arc::new(artifacts);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let slow = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 8192];
        let received = stream.read(&mut request).await.unwrap();
        assert!(received > 0);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: 100000\r\n\r\npartial").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    let url = format!("{origin}/song.mp3");
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        service.import_remote(
            "owner",
            "session",
            "run",
            RemoteFile {
                service_url: &origin,
                url: &url,
                filename: "song.mp3",
                size: None,
                sha256: None,
            },
        ),
    )
    .await;
    assert!(result.is_err());
    slow.abort();
    assert_eq!(
        std::fs::read_dir(root.path().join("files/objects"))
            .unwrap()
            .count(),
        0
    );
}
