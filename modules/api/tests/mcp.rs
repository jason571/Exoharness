//! Original management contracts over HTTP + PostgreSQL + standard MCP mock.
#[path = "../../mcp/tests/support/mod.rs"]
#[allow(dead_code)]
mod protocol;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{
    ApiState, ExecutionService, Surface, auth::Credentials, mcp_router, registry_router,
};
use exoharness_mcp::Mcp;
use exoharness_registry::Registry;
use exoharness_secrets::SecretStore;
use exoharness_skills::Skills;
use exoharness_storage_postgres::{PostgresStore, RegistryAccess};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;
use tower::ServiceExt;
const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu";
const READER: &str = "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr";
const ACCESS: RegistryAccess = RegistryAccess::Administrator;
struct Fixture {
    _root: tempfile::TempDir,
    router: Router,
    mcp: Arc<Mcp>,
}
async fn fixture() -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let store = tokio::task::spawn_blocking(move || {
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        let store = PostgresStore::connect(
            &url,
            &format!("pro_mcp_http_{}_{}", std::process::id(), number),
        )
        .unwrap();
        store.migrate_registry().unwrap();
        store
    })
    .await
    .unwrap();
    let registry = Arc::new(Registry(store.clone()));
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let secrets = Arc::new(SecretStore::open(root.path().join("secrets")).unwrap());
    let copy = registry.clone();
    let path = root.path().to_path_buf();
    let skills = Arc::new(
        tokio::task::spawn_blocking(move || Skills::open(path, copy).unwrap())
            .await
            .unwrap(),
    );
    let mcp = Arc::new(Mcp::new(registry.clone(), secrets));
    let credentials=Arc::new(Credentials::from_json(&json!([{"id":"admin","role":"admin","token":ADMIN},{"id":"user","role":"user","token":USER},{"id":"reader","role":"chat","token":READER}]).to_string()).unwrap());
    let (_, stop) = watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let router = mcp_router(state.clone(), mcp.clone()).merge(registry_router(
        state,
        registry,
        skills,
        mcp.clone(),
    ));
    Fixture {
        _root: root,
        router,
        mcp,
    }
}
async fn call(
    f: &Fixture,
    method: &str,
    path: &str,
    token: &str,
    input: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:9797")
        .header("origin", "http://127.0.0.1:9797")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(input.to_string()))
        .unwrap();
    let response = f.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn input(url: &str) -> Value {
    json!({"content":json!({"mcpServers":{"fixture":{"type":"streamableHttp","url":url,"headers":{"Authorization":"Token test-secret"}}}}).to_string(),"scope":{"kind":"global"},"actor":"spoofed"})
}
#[tokio::test]
async fn pure_import_authorization_discovery_rotation_and_deletion_preserve_old_contracts() {
    let f = fixture().await;
    let server = protocol::server(protocol::StateData::default()).await;
    for token in [USER, READER] {
        assert_eq!(
            call(
                &f,
                "POST",
                "/v1/imports/manifest",
                token,
                input(&server.url)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, imported) = call(
        &f,
        "POST",
        "/v1/imports/manifest",
        ADMIN,
        input(&server.url),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert!(!imported.to_string().contains("test-secret"));
    assert!(server.state.requests.lock().unwrap().is_empty());
    let id = imported["capabilities"][0]["id"].as_str().unwrap();
    let path = format!("/v1/capabilities/{id}");
    let (_, secrets) = call(&f, "GET", &format!("{path}/secrets"), READER, json!({})).await;
    assert_eq!(secrets[0]["configured"], true);
    assert!(secrets[0].get("key").is_none());
    let (status, denied) = call(
        &f,
        "POST",
        &format!("{path}/validate"),
        ADMIN,
        json!({"confirm":true}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert!(server.state.requests.lock().unwrap().is_empty());
    assert_eq!(
        call(
            &f,
            "PUT",
            &format!("{path}/permissions/network.egress"),
            ADMIN,
            json!({"decision":"allow","expected_revision":1})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, prepared) = call(
        &f,
        "POST",
        &format!("{path}/preparation"),
        ADMIN,
        json!({"expected_revision":1,"confirm":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{prepared}");
    assert_eq!(prepared["status"], "ready");
    let (_, tools) = call(&f, "GET", &format!("{path}/tools"), READER, json!({})).await;
    assert_eq!(tools.as_array().unwrap().len(), 2);
    assert_eq!(tools[0]["inputSchema"]["required"][0], "text");
    assert!(
        server
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r["method"] != "tools/call")
    );
    assert!(
        server
            .state
            .headers
            .lock()
            .unwrap()
            .iter()
            .all(|h| h["authorization"] == "Token test-secret")
    );
    let mcp = f.mcp.clone();
    let owned = id.to_owned();
    let previous =
        tokio::task::spawn_blocking(move || mcp.registry.0.capability(&ACCESS, &owned).unwrap())
            .await
            .unwrap();
    let old_key = previous.manifest.secret_refs[0].key.clone();
    let name = secrets[0]["name"].as_str().unwrap();
    let secret_path = format!("{path}/secrets/{name}");
    assert_eq!(
        call(&f, "PUT", &secret_path, READER, json!({"value":"other"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, rotated) = call(
        &f,
        "PUT",
        &secret_path,
        ADMIN,
        json!({"value":"Token rotated","expected_revision":2}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    assert_eq!(rotated["revision"], 3);
    assert!(!rotated.to_string().contains("rotated"));
    assert_eq!(
        f.mcp.secrets.get(&old_key).unwrap().expose(),
        "Token test-secret"
    );
    let (_, stale) = call(&f, "GET", &format!("{path}/tools"), ADMIN, json!({})).await;
    assert_eq!(stale, json!([]));
    assert_eq!(
        call(
            &f,
            "POST",
            &format!("{path}/validate"),
            ADMIN,
            json!({"confirm":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, cleared) = call(
        &f,
        "DELETE",
        &secret_path,
        ADMIN,
        json!({"expected_revision":3}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert_eq!(cleared["configured"], false);
    let (_, blocked) = call(
        &f,
        "POST",
        &format!("{path}/validate"),
        ADMIN,
        json!({"confirm":true}),
    )
    .await;
    assert_eq!(blocked["error"]["code"], "secret_required");
    assert_eq!(
        call(&f, "DELETE", &path, ADMIN, json!({"expected_revision":4}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&f, "GET", &path, ADMIN, json!({})).await.0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn multi_server_conflict_and_invalid_json_never_commit_partial_packages() {
    let f = fixture().await;
    let server = protocol::server(protocol::StateData::default()).await;
    assert_eq!(
        call(
            &f,
            "POST",
            "/v1/imports/manifest",
            ADMIN,
            input(&server.url)
        )
        .await
        .0,
        StatusCode::OK
    );
    let before = std::fs::read_dir(f._root.path().join("secrets"))
        .unwrap()
        .count();
    let content=json!({"mcpServers":{"another":{"url":server.url,"headers":{"Authorization":"new-value"}},"fixture":{"url":server.url,"headers":{"Authorization":"new-value"}}}}).to_string();
    assert_eq!(
        call(
            &f,
            "POST",
            "/v1/imports/manifest",
            ADMIN,
            json!({"content":content})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        std::fs::read_dir(f._root.path().join("secrets"))
            .unwrap()
            .count(),
        before
    );
    let (_, records) = call(&f, "GET", "/v1/capabilities", ADMIN, json!({})).await;
    assert_eq!(records.as_array().unwrap().len(), 1);
    for content in [
        "{\"mcpServers\":{},\"mcpServers\":{}}",
        "{\"mcpServers\":{\"invalid\":{\"url\":\"http://127.0.0.1:1/mcp\",\"headers\":{\"Authorization\":\"a\\nb\"}}}}",
    ] {
        assert_eq!(
            call(
                &f,
                "POST",
                "/v1/imports/manifest",
                ADMIN,
                json!({"content":content})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert!(server.state.requests.lock().unwrap().is_empty());
}
#[tokio::test]
async fn permission_withdrawal_and_failed_discovery_cannot_expose_stale_schema() {
    let f = fixture().await;
    let server = protocol::server(protocol::StateData {
        repeated_cursor: true,
        ..Default::default()
    })
    .await;
    let (_, imported) = call(
        &f,
        "POST",
        "/v1/imports/manifest",
        ADMIN,
        input(&server.url),
    )
    .await;
    let id = imported["capabilities"][0]["id"].as_str().unwrap();
    let path = format!("/v1/capabilities/{id}");
    call(
        &f,
        "PUT",
        &format!("{path}/permissions/network.egress"),
        ADMIN,
        json!({"decision":"allow"}),
    )
    .await;
    let (status, result) = call(
        &f,
        "POST",
        &format!("{path}/validate"),
        ADMIN,
        json!({"confirm":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["status"], "failed");
    let (_, health) = call(&f, "GET", &format!("{path}/health"), ADMIN, json!({})).await;
    assert_eq!(health["status"], "unavailable");
    assert_eq!(
        call(&f, "GET", &format!("{path}/tools"), ADMIN, json!({}))
            .await
            .1,
        json!([])
    );
    call(
        &f,
        "PUT",
        &format!("{path}/permissions/network.egress"),
        ADMIN,
        json!({"decision":"deny"}),
    )
    .await;
    let count = server.state.requests.lock().unwrap().len();
    assert_eq!(
        call(
            &f,
            "POST",
            &format!("{path}/preparation"),
            ADMIN,
            json!({"expected_revision":1,"confirm":true})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(server.state.requests.lock().unwrap().len(), count);
}
