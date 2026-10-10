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
use std::sync::Arc;
use tower::ServiceExt;
async fn fixture() -> (tempfile::TempDir, Arc<Models>, Router) {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let models = Arc::new(Models::initialize(root.path()).unwrap());
    let store = tokio::task::spawn_blocking(|| {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let namespace = format!(
            "pro_model_writes_http_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        PostgresStore::connect(
            &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated database required"),
            &namespace,
        )
        .unwrap()
    })
    .await
    .unwrap();
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"admin","role":"admin","token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                {"id":"user","role":"user","token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
                        {"id":"readonly","role":"chat","token":"cccccccccccccccccccccccccccccccc"}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let (_stop, stop) = tokio::sync::watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let router = configuration_router(state, models.clone(), Arc::new(Assistants::empty()));
    (root, models, router)
}
fn value() -> Value {
    json!({"actor":"untrusted-request-actor","api_key":"private-api-test-key","provider":{"name":"mock-api","model_type":"llm","api":"openai_compatible","base_url":"https://example.test/v1","enabled":true,"api_key_configured":false},"model":{"name":"model-a","model_id":"mock-model","display_name":"Mock","provider":"mock-api","model_type":"llm","context_window":128000,"max_tokens":4096,"thinking":"none","enabled":true}})
}
async fn request(
    router: &Router,
    method: &str,
    path: &str,
    body: Value,
    token: &str,
) -> (StatusCode, Value) {
    raw(router, method, path, body.to_string(), token).await
}
async fn raw(
    router: &Router,
    method: &str,
    path: &str,
    body: String,
    token: &str,
) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("host", "127.0.0.1:9797")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}
#[tokio::test]
async fn original_management_save_switch_rotate_delete_and_restart_contracts_work() {
    let (root, models, router) = fixture().await;
    let admin = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let (status, saved) = request(&router, "POST", "/v1/model-configuration", value(), admin).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!saved.to_string().contains("private-api-test-key"));
    assert_eq!(saved["provider"]["api_key_configured"], true);
    let mut second = value()["model"].clone();
    second["name"] = json!("model-b");
    assert_eq!(
        request(
            &router,
            "POST",
            "/v1/models",
            json!({"model":second}),
            admin
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(models.catalog()["models"][0]["enabled"], false);
    assert_eq!(
        request(
            &router,
            "PUT",
            "/v1/models/model-a/default",
            json!({"actor":"web-console"}),
            admin
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(models.catalog()["default_model"], "model-a");
    let mut rotated = value();
    rotated["api_key"] = json!("rotated-test-key");
    assert_eq!(
        request(&router, "POST", "/v1/model-configuration", rotated, admin)
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        Models::open(root.path()).unwrap().catalog(),
        models.catalog()
    );
    assert_eq!(
        request(
            &router,
            "DELETE",
            "/v1/model-providers/mock-api",
            json!({"confirm":true}),
            admin
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &router,
            "DELETE",
            "/v1/models/model-a",
            json!({"confirm":false}),
            admin
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for name in ["model-a", "model-b"] {
        assert_eq!(
            request(
                &router,
                "DELETE",
                &format!("/v1/models/{name}"),
                json!({"confirm":true}),
                admin
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        request(
            &router,
            "DELETE",
            "/v1/model-providers/mock-api",
            json!({"confirm":true}),
            admin
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        Models::open(root.path()).unwrap().catalog()["models"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn malformed_native_fields_duplicates_stale_revisions_and_roles_do_not_write() {
    let (_root, models, router) = fixture().await;
    let admin = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    for token in [
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccc",
    ] {
        assert_eq!(
            request(&router, "POST", "/v1/model-configuration", value(), token)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let mut invalid = value();
    invalid["model"]["max_tokens"] = json!("4096");
    assert_eq!(
        request(&router, "POST", "/v1/model-configuration", invalid, admin)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert!(models.catalog()["models"].as_array().unwrap().is_empty());
    let duplicate =
        value()
            .to_string()
            .replacen("\"api_key\":", "\"api_key\":\"duplicate\",\"api_key\":", 1);
    assert_eq!(
        raw(&router, "POST", "/v1/model-configuration", duplicate, admin)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(models.revision(), 0);
    request(&router, "POST", "/v1/model-configuration", value(), admin).await;
    let before = models.catalog();
    assert_eq!(
        request(
            &router,
            "PUT",
            "/v1/models/model-a/enabled",
            json!({"enabled":false,"expected_revision":0}),
            admin
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &router,
            "PUT",
            "/v1/models/model-a/enabled",
            json!({"enabled":"false"}),
            admin
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(models.catalog(), before);
    let (status, failure) = request(
        &router,
        "POST",
        "/v1/models/model-a/test",
        json!({"confirm":true}),
        admin,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(failure["resumable"], false);
}
