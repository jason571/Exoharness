#[path = "../../artifacts/tests/support.rs"]
#[allow(dead_code)]
mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::{collections::BTreeMap, sync::Arc};
use tower::ServiceExt;
#[test]
fn forged_manifest_cannot_grant_another_runs_asset() {
    let (_root, store, artifacts) = support::fixture();
    let snapshot = store.run("owner", "session", "run").unwrap().snapshot;
    store.create_session("other", "foreign").unwrap();
    store
        .create_turn(exoharness_storage_postgres::CreateTurn {
            owner: "other",
            session_id: "foreign",
            expected_session_version: 0,
            turn_id: "foreign-turn",
            run_id: "foreign-run",
            input: "fixture",
            snapshot: &snapshot,
        })
        .unwrap();
    let foreign = artifacts
        .publish_file(
            "other",
            "foreign",
            "foreign-run",
            "index.html",
            support::file(b"private"),
        )
        .unwrap();
    let manifest = serde_json::json!({"version":1,"entrypoint":"index.html","members":{"index.html":{"id":foreign.artifact.id,"filename":"index.html"}}});
    let published = artifacts
        .publish_file(
            "owner",
            "session",
            "run",
            "project-preview.json",
            support::file(manifest.to_string().as_bytes()),
        )
        .unwrap();
    let (_, signature) = support::access(&published.url);
    assert!(
        artifacts
            .open_project_signed(&published.artifact.id, &signature, "index.html")
            .is_err()
    );
    assert!(
        artifacts
            .open_project_signed(&published.artifact.id, &signature, "../index.html")
            .is_err()
    );
}
#[test]
fn static_members_keep_relative_paths_and_require_one_signed_immutable_manifest() {
    let (_root, _store, artifacts) = support::fixture();
    let card=artifacts.publish_project("owner","session","run",Some("pages/index.html"),BTreeMap::from([
        ("pages/index.html".into(),support::file(b"<link rel='stylesheet' href='../assets/site.css'><script src='../assets/app.js'></script>")),
        ("assets/site.css".into(),support::file(b"body {color: red;}")),
        ("assets/app.js".into(),support::file(b"console.log('ok');")),
    ])).unwrap();
    let url = url::Url::parse(card["preview_url"].as_str().unwrap()).unwrap();
    let router = exoharness_api::media_router(Arc::new(artifacts), 9790);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        for (path, mime) in [
            (url.clone(), "text/html"),
            (url.join("../assets/site.css").unwrap(), "text/css"),
            (url.join("../assets/app.js").unwrap(), "text/javascript"),
        ] {
            let r = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path.path())
                        .header("host", "127.0.0.1:9790")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::OK);
            assert!(
                r.headers()["content-type"]
                    .to_str()
                    .unwrap()
                    .starts_with(mime)
            );
            let csp = r.headers()["content-security-policy"].to_str().unwrap();
            assert!(csp.contains("connect-src http://127.0.0.1:9790/projects/"));
            assert!(!csp.contains("allow-same-origin"));
            assert!(!r.into_body().collect().await.unwrap().to_bytes().is_empty());
        }
        for path in [
            url.join("../private.py").unwrap().path().to_owned(),
            url.path().replace("/projects/", "/projects/unknown/"),
            url.path().replace("/pages/index.html", "/%2e%2e/private"),
        ] {
            let r = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("host", "127.0.0.1:9790")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::NOT_FOUND);
        }
        let download = url::Url::parse(card["download_url"].as_str().unwrap()).unwrap();
        let r = router
            .oneshot(
                Request::builder()
                    .uri(format!("{}?{}", download.path(), download.query().unwrap()))
                    .header("host", "127.0.0.1:9790")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(
            r.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .starts_with(b"PK")
        );
    });
}
