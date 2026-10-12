//! Explicit real-browser acceptance, no paid model or image build.
#[path = "../../artifacts/tests/support.rs"]
#[allow(dead_code)]
mod support;
use std::{collections::BTreeMap, sync::Arc};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the cached skill-workspace-browser image and explicit isolated test database"]
async fn multi_page_site_loads_relative_css_modules_images_and_interaction_in_real_chromium() {
    let (_root, store, _artifacts) = tokio::task::spawn_blocking(support::fixture).await.unwrap();
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let service = exoharness_artifacts::Artifacts::open(
        _root.path().join("browser-project"),
        store,
        &format!("http://127.0.0.1:{port}"),
    )
    .unwrap();
    let sources = BTreeMap::from([
        (
            "index.html".into(),
            support::file(include_bytes!("fixtures/coding_project/index.html")),
        ),
        (
            "pages/details.html".into(),
            support::file(include_bytes!("fixtures/coding_project/pages/details.html")),
        ),
        (
            "assets/style.css".into(),
            support::file(include_bytes!("fixtures/coding_project/assets/style.css")),
        ),
        (
            "assets/app.mjs".into(),
            support::file(include_bytes!("fixtures/coding_project/assets/app.mjs")),
        ),
        (
            "assets/cover.svg".into(),
            support::file(include_bytes!("fixtures/coding_project/assets/cover.svg")),
        ),
    ]);
    let (service, card) = tokio::task::spawn_blocking(move || {
        let card = service
            .publish_project("owner", "session", "run", Some("index.html"), sources)
            .unwrap();
        (service, card)
    })
    .await
    .unwrap();
    let router = exoharness_api::media_router(Arc::new(service), port);
    let server = tokio::spawn(async move { axum::serve(socket, router).await.unwrap() });
    let source_root = std::env::var_os("EXOHARNESS_PRO_TEST_SOURCE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let script = source_root
        .join("scripts/tests/coding_browser.py")
        .canonicalize()
        .unwrap();
    let url = card["preview_url"].as_str().unwrap().to_owned();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("docker")
            .args([
                "run",
                "--pull=never",
                "--rm",
                "--network",
                "host",
                "--read-only",
                "--user",
                "65532:65532",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges",
                "--memory",
                "2g",
                "--cpus",
                "2",
                "--pids-limit",
                "256",
                "--tmpfs",
                "/tmp:rw,nosuid,size=512m,mode=1777",
                "--mount",
            ])
            .arg(format!(
                "type=bind,src={},dst=/test.py,readonly",
                script.display()
            ))
            .arg("-e")
            .arg(format!("EXOHARNESS_CODING_TEST_URL={url}"))
            .args([
                "--entrypoint",
                "python",
                "skill-workspace-browser:0.1.0",
                "-B",
                "/test.py",
            ])
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.abort();
    assert!(
        output.status.success(),
        "browser acceptance failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
