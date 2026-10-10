use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{
    ApiState, ExecutionService, Surface, auth::Credentials, configuration_router,
};
use exoharness_configuration::{Assistants, Models};
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::watch;
use tower::ServiceExt;

#[tokio::test]
async fn imported_configuration_routes_are_administrator_only_and_legacy_types_remain_native() {
    let store = tokio::task::spawn_blocking(|| {
        let url =
            std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated database required");
        PostgresStore::connect(&url, &format!("pro_config_http_{}", std::process::id())).unwrap()
    })
    .await
    .unwrap();
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"admin","role":"admin","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                {"id":"user","role":"user","token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let (_stop, stop) = watch::channel(false);
    let base = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let admin = configuration_router(
        base.clone(),
        Arc::new(Models::empty()),
        Arc::new(Assistants::empty()),
    );
    for (token, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            StatusCode::FORBIDDEN,
        ),
        (Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), StatusCode::OK),
    ] {
        let mut request = Request::builder()
            .uri("/v1/model-catalog")
            .header("host", "127.0.0.1:9797");
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = admin
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        if status == StatusCode::OK {
            assert_eq!(response.headers()["cache-control"], "no-store");
            let value: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert!(value["models"].is_array() && value["providers"].is_array());
            assert!(value.get("api_key").is_none());
        }
    }
    let portal = configuration_router(
        ApiState {
            surface: Surface::Portal,
            port: 9798,
            ..base
        },
        Arc::new(Models::empty()),
        Arc::new(Assistants::empty()),
    );
    let request = Request::builder()
        .uri("/v1/model-catalog")
        .header("host", "127.0.0.1:9798")
        .header("authorization", "Bearer bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        portal.clone().oneshot(request).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let request = Request::builder()
        .uri("/v1/agent-profiles/default")
        .header("host", "127.0.0.1:9798")
        .header("authorization", "Bearer bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .body(Body::empty())
        .unwrap();
    let response = portal.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(value.is_null());
}
