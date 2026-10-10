#[path = "../../artifacts/tests/support.rs"]
#[allow(dead_code)]
mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::Arc;
use support::{file, fixture};
use tower::ServiceExt;

#[test]
fn metadata_requires_the_same_signature_and_never_exposes_private_paths() {
    let (_root, _, artifacts) = fixture();
    let published = artifacts
        .publish_file("owner", "session", "run", "voice.mp3", file(b"sound"))
        .unwrap();
    let mut url = url::Url::parse(&published.url).unwrap();
    url.set_path(&format!("{}/metadata", url.path()));
    let router = exoharness_api::media_router(Arc::new(artifacts), 9790);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        for (query, expected) in [
            (url.query().unwrap().to_string(), StatusCode::OK),
            ("expires=0&signature=bad".into(), StatusCode::NOT_FOUND),
            ("expires=0&signature=".into(), StatusCode::NOT_FOUND),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("{}?{query}", url.path()))
                        .header("host", "127.0.0.1:9790")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            let body = response.into_body().collect().await.unwrap().to_bytes();
            if expected == StatusCode::OK {
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(value["artifact_id"], published.artifact.id);
                assert_eq!(value["size_bytes"], 5);
                assert!(value.get("relative_path").is_none());
                assert!(value["provenance"].is_null());
            }
        }
    });
}
