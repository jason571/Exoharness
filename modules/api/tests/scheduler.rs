use axum::{
    Router,
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
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tower::ServiceExt;
const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
async fn fixture() -> (Router, Router, PostgresStore) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ns = format!(
        "pro_scheduler_http_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = tokio::task::spawn_blocking(move || {
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        let s = PostgresStore::connect(&url, &ns).unwrap();
        s.migrate().unwrap();
        s
    })
    .await
    .unwrap();
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"admin","role":"admin","token":ADMIN},{"id":"user","role":"user","token":USER}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let (_, stop) = tokio::sync::watch::channel(false);
    let base = ApiState {
        execution: Arc::new(ExecutionService(store.clone())),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    (
        configuration_router(
            base.clone(),
            Arc::new(Models::empty()),
            Arc::new(Assistants::empty()),
        ),
        configuration_router(
            ApiState {
                surface: Surface::Portal,
                port: 9798,
                ..base
            },
            Arc::new(Models::empty()),
            Arc::new(Assistants::empty()),
        ),
        store,
    )
}
async fn request(
    router: &Router,
    port: u16,
    token: Option<&str>,
    method: &str,
    body: String,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri("/v1/scheduler-configuration")
        .header("host", format!("127.0.0.1:{port}"))
        .header("content-type", "application/json");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    let result = router
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = result.status();
    let bytes = result.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn scheduler_api_authorization_defaults_save_and_conflict_are_real() {
    let (admin, portal, store) = fixture().await;
    assert_eq!(
        request(&admin, 9797, None, "GET", String::new()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&admin, 9797, Some(USER), "GET", String::new())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&portal, 9798, Some(USER), "GET", String::new())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, mut value) = request(&admin, 9797, Some(ADMIN), "GET", String::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["max_active_runs_per_user"], 2);
    assert_eq!(value["max_global_agent_runs"], 1000);
    assert_eq!(value["mcp_capabilities"], json!([]));
    value.as_object_mut().unwrap().remove("mcp_capabilities");
    value["max_active_runs_per_user"] = json!(3);
    value["max_global_agent_runs"] = json!(8);
    let (status, saved) = request(&admin, 9797, Some(ADMIN), "PUT", value.to_string()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["revision"], 2);
    assert_eq!(
        request(&admin, 9797, Some(ADMIN), "PUT", value.to_string())
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&admin, 9797, Some(USER), "PUT", value.to_string())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        tokio::task::spawn_blocking(move || store
            .scheduler_configuration()
            .unwrap()
            .max_global_agent_runs)
        .await
        .unwrap(),
        8
    );
}
#[tokio::test]
async fn invalid_limits_duplicate_fields_and_unregistered_assignments_cannot_be_saved() {
    let (router, _, _) = fixture().await;
    let (_, mut value) = request(&router, 9797, Some(ADMIN), "GET", String::new()).await;
    value.as_object_mut().unwrap().remove("mcp_capabilities");
    let mut bad = value.clone();
    bad["max_active_runs_per_user"] = json!(0);
    assert_eq!(
        request(&router, 9797, Some(ADMIN), "PUT", bad.to_string())
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let duplicate = value.to_string().replacen("{", "{\"revision\":0,", 1);
    assert_eq!(
        request(&router, 9797, Some(ADMIN), "PUT", duplicate)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    value["resource_pools"] = json!([{"id":"gpu","name":"GPU","max_concurrency":1}]);
    value["mcp_resource_assignments"] = json!({"unregistered":"gpu"});
    assert_eq!(
        request(&router, 9797, Some(ADMIN), "PUT", value.to_string())
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, unchanged) = request(&router, 9797, Some(ADMIN), "GET", String::new()).await;
    assert_eq!(unchanged["revision"], 1);
}
