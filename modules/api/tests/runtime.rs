use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials, runtime_router};
use exoharness_sandbox::Sandbox;
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::watch;
use tower::ServiceExt;
const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu";
#[tokio::test]
async fn runtime_fact_projection_does_not_grant_execution_or_claim_agent_readiness() {
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated PostgreSQL required");
    let store = tokio::task::spawn_blocking(move || {
        PostgresStore::connect(&url, "pro_runtime_fact_http").unwrap()
    })
    .await
    .unwrap();
    let credentials=Arc::new(Credentials::from_json(&json!([{"id":"admin","role":"admin","token":ADMIN},{"id":"user","role":"user","token":USER}]).to_string()).unwrap());
    let (_, stop) = watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let image =
        std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE").expect("prepared local image required");
    let sandbox = Arc::new(Sandbox::open(root.path().into(), &image).await.unwrap());
    for (backend, ready) in [(None, false), (Some(sandbox.clone()), true)] {
        let router = runtime_router(state.clone(), backend);
        for (key, expected) in [(ADMIN, StatusCode::OK), (USER, StatusCode::FORBIDDEN)] {
            let request = Request::builder()
                .uri("/v1/local/runtime")
                .header("host", "127.0.0.1:9797")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap();
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), expected);
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            if expected == StatusCode::OK {
                assert_eq!(value["sandbox_ready"], ready);
                assert_eq!(value["agent_integrated"], false);
                assert_eq!(value["network"], "disabled");
                assert_eq!(value["runtimes"].as_array().unwrap().len(), 4);
            }
        }
    }
    sandbox.stop().await.unwrap();
}
