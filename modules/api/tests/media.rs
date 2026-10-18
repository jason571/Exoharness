#[path = "../../artifacts/tests/support.rs"]
#[allow(dead_code)]
mod support;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::Arc;
use support::{file, fixture};
use tower::ServiceExt;
async fn fetch(
    router: &Router,
    url: &str,
    method: &str,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let url = url::Url::parse(url).unwrap();
    let mut request = Request::builder()
        .method(method)
        .uri(format!("{}?{}", url.path(), url.query().unwrap()))
        .header("host", "127.0.0.1:9790");
    if let Some(value) = range {
        request = request.header("range", value);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, body)
}
#[test]
fn media_playback_ranges_heads_and_download_share_one_permanent_signed_object() {
    let (_root, _, artifacts) = fixture();
    let published = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"0123456789"))
        .unwrap();
    let router = exoharness_api::media_router(Arc::new(artifacts), 9790);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (status, headers, body) = fetch(&router, &published.url, "GET", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"0123456789");
        assert_eq!(headers["content-type"], "video/mp4");
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("inline")
        );
        for (range, expected, content) in [
            ("bytes=2-5", &b"2345"[..], "bytes 2-5/10"),
            ("bytes=7-", &b"789"[..], "bytes 7-9/10"),
            ("bytes=-3", &b"789"[..], "bytes 7-9/10"),
        ] {
            let (status, headers, body) = fetch(&router, &published.url, "GET", Some(range)).await;
            assert_eq!(status, StatusCode::PARTIAL_CONTENT);
            assert_eq!(body, expected);
            assert_eq!(headers["content-range"], content);
        }
        let (status, headers, body) =
            fetch(&router, &published.url, "HEAD", Some("bytes=0-1")).await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(headers["content-length"], "2");
        assert!(body.is_empty());
        for range in ["bytes=10-", "bytes=7-2", "bytes=0-1,3-4", "bytes=-0"] {
            let (status, headers, body) = fetch(&router, &published.url, "GET", Some(range)).await;
            assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
            assert_eq!(headers["content-range"], "bytes */10");
            assert!(body.is_empty());
        }
        let (_, headers, _) = fetch(&router, &published.download_url, "GET", None).await;
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("attachment")
        );
        let invalid = published.url.replace("signature=", "signature=x");
        assert_eq!(
            fetch(&router, &invalid, "GET", None).await.0,
            StatusCode::NOT_FOUND
        );
    });
}
#[test]
fn generated_html_has_a_separate_sandboxed_preview_and_old_workspace_routes_work() {
    let (_root, _, artifacts) = fixture();
    let published = artifacts
        .publish_file(
            "owner",
            "session",
            "run",
            "中文页面.html",
            file("<!doctype html><h1>中文预览</h1>".as_bytes()),
        )
        .unwrap();
    let router = exoharness_api::media_router(Arc::new(artifacts), 9790);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (status, headers, body) = fetch(
            &router,
            published.preview_url.as_ref().unwrap(),
            "GET",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("inline")
        );
        assert!(
            headers["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("sandbox allow-scripts")
        );
        assert_eq!(body, "<!doctype html><h1>中文预览</h1>".as_bytes());
        let csp = headers["content-security-policy"].to_str().unwrap();
        assert!(csp.contains("allow-popups allow-popups-to-escape-sandbox"));
        assert!(!csp.contains("allow-same-origin"));
        assert!(!csp.contains("allow-forms"));
        assert!(csp.contains("connect-src 'none'") && csp.contains("form-action 'none'"));
        if let Ok(prefix) = std::env::var("EXOHARNESS_PREVIEW_CSP_FIXTURE") {
            std::fs::write(
                format!("{prefix}-single.json"),
                serde_json::json!({"csp":csp}).to_string(),
            )
            .unwrap();
        }
        let url = published.url.replace("/artifacts/", "/workspace-files/");
        let (status, headers, _) = fetch(&router, &url, "GET", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .starts_with("attachment")
        );
        assert!(
            headers["content-disposition"]
                .to_str()
                .unwrap()
                .contains("filename*=UTF-8''%")
        );
    });
}
