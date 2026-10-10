use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_api::{
    ApiState, ExecutionService, Surface, auth::Credentials, registry_router, skills_router,
};
use exoharness_registry::Registry;
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
const READONLY: &str = "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr";
const USER: &str = "uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu";
#[tokio::test]
async fn assistant_projection_uses_original_tool_kind_without_changing_registry_records() {
    use exoharness_contracts::capability::{
        CapabilityKind, CapabilityManifest, ValidatedCapability,
    };
    let f = fixture().await;
    let manifest:CapabilityManifest=serde_json::from_value(json!({"schema_version":1,"name":"sandbox-test-tool","version":"1.0.0","description":"Test isolated tool registration","scope":{"kind":"global"},"source":{"kind":"uploaded","file_name":"test.json"},"permissions":[],"secret_refs":[],"spec":{"kind":"tool_node","config":{"tools":[{"name":"test","description":"Fixture","input_schema":{"type":"object"}}],"runner":{"type":"oci","image":"test-only:no-execution","args":[]},"sandbox":"oci"}}})).unwrap();
    let registry = f.registry.clone();
    let record = tokio::task::spawn_blocking(move || {
        registry
            .0
            .register_capability(
                &RegistryAccess::Administrator,
                ValidatedCapability::new(manifest, None).unwrap(),
                "test",
                false,
            )
            .unwrap()
    })
    .await
    .unwrap();
    let (status, view) = raw(
        &f.router,
        "GET",
        "/v1/assistant-capabilities",
        Some(ADMIN),
        String::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let projected = view
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == record.id)
        .unwrap();
    assert_eq!(projected["manifest"]["spec"]["kind"], "tool");
    assert_eq!(projected["availability"]["available"], false);
    let (_, canonical) = raw(
        &f.router,
        "GET",
        "/v1/capabilities/overview",
        Some(ADMIN),
        String::new(),
    )
    .await;
    assert_eq!(
        canonical
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == record.id)
            .unwrap()["manifest"]["spec"]["kind"],
        "tool_node"
    );
    let store = f.registry.0.clone();
    assert_eq!(
        tokio::task::spawn_blocking(move || store
            .capability(&RegistryAccess::Administrator, &record.id)
            .unwrap()
            .manifest
            .kind())
        .await
        .unwrap(),
        CapabilityKind::ToolNode
    );
}
struct Fixture {
    _root: tempfile::TempDir,
    router: Router,
    portal: Router,
    registry: Arc<Registry>,
    sandbox: Option<Arc<exoharness_sandbox::Sandbox>>,
}
async fn fixture() -> Fixture {
    fixture_with_runtime(false).await
}
async fn fixture_with_runtime(with_runtime: bool) -> Fixture {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let store = tokio::task::spawn_blocking(move || {
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        let store = PostgresStore::connect(
            &url,
            &format!("pro_skill_http_{}_{}", std::process::id(), number),
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
    let path = root.path().to_path_buf();
    let registry_copy = registry.clone();
    let skills = Arc::new(
        tokio::task::spawn_blocking(move || Skills::open(path, registry_copy).unwrap())
            .await
            .unwrap(),
    );
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
        {"id":"admin","role":"admin","token":ADMIN},
        {"id":"reader","role":"chat","token":READONLY},
        {"id":"user","role":"user","token":USER}])
            .to_string(),
        )
        .unwrap(),
    );
    let (_, stop) = watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let secrets =
        Arc::new(exoharness_secrets::SecretStore::open(root.path().join("secrets")).unwrap());
    let mcp = Arc::new(exoharness_mcp::Mcp::new(registry.clone(), secrets));
    let sandbox = if with_runtime {
        Some(Arc::new(
            exoharness_sandbox::Sandbox::open(
                root.path().join("sandbox"),
                &std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE").expect("cached image"),
            )
            .await
            .unwrap(),
        ))
    } else {
        None
    };
    let runtime = sandbox.as_ref().map(|sandbox| {
        Arc::new(
            exoharness_skill_runtime::SkillRuntime::open(
                root.path().join("environment"),
                skills.clone(),
                registry.clone(),
                sandbox.clone(),
            )
            .unwrap(),
        )
    });
    let router = if with_runtime {
        exoharness_api::registry_router_with_runtime(
            state.clone(),
            registry.clone(),
            skills.clone(),
            mcp,
            runtime,
        )
    } else {
        registry_router(state.clone(), registry.clone(), skills.clone(), mcp)
    }
    .merge(skills_router(state.clone(), skills.clone()));
    let portal = skills_router(
        ApiState {
            surface: Surface::Portal,
            port: 9798,
            ..state
        },
        skills,
    );
    Fixture {
        _root: root,
        router,
        portal,
        registry,
        sandbox,
    }
}
async fn raw(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: String,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:9797")
        .header("origin", "http://127.0.0.1:9797")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let reply = router
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = reply.status();
    let bytes = reply.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn call(f: &Fixture, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    raw(&f.router, method, path, Some(ADMIN), body.to_string()).await
}
fn package(body: &str) -> Value {
    json!({"package_kind":"skill","file_name":"fixture","actor":"spoofed","files":[
        {"path":"fixture/SKILL.md","content_base64":STANDARD.encode(format!("---\nname: http-fixture\ndescription: HTTP fixture instructions\n---\n{body}"))},
        {"path":"fixture/references/data.txt","content_base64":STANDARD.encode("x".repeat(70_000))}]})
}
#[tokio::test]
async fn skill_preparation_cannot_enable_a_skill_with_unapproved_permissions() {
    let f = fixture().await;
    let mut input = package("Permission check only.");
    input["permissions"] = json!(["workspace.read"]);
    let (status, imported) = call(&f, "POST", "/v1/imports/skill", input).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    let record = &imported["capabilities"][0];
    let path = format!("/v1/capabilities/{}", record["id"].as_str().unwrap());
    let (status, _) = call(
        &f,
        "POST",
        &format!("{path}/preparation"),
        json!({"confirm":true,"expected_revision":record["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, current) = call(&f, "GET", &path, Value::Null).await;
    assert_eq!(current["enabled"], false);
    assert_eq!(current["revision"], record["revision"]);
}
#[tokio::test]
async fn skill_preparation_checks_the_cached_isolated_environment_without_executing_package_code() {
    let f = fixture_with_runtime(true).await;
    let mut input = package("Run only after separate authorization.");
    input["files"].as_array_mut().unwrap().push(json!({"path":"fixture/main.py","content_base64":STANDARD.encode("raise RuntimeError('not on import or preparation')")}));
    let (status, response) = call(&f, "POST", "/v1/imports/skill", input).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let record = &response["capabilities"][0];
    let id = record["id"].as_str().unwrap();
    let revision = record["revision"].as_i64().unwrap();
    let path = format!("/v1/capabilities/{id}");
    let before = call(&f, "GET", &format!("{path}/preparation"), Value::Null)
        .await
        .1;
    assert_eq!(before["status"], "blocked");
    assert_eq!(before["runtime_ready"], false);
    assert_eq!(before["instructions_ready"], true);
    assert_eq!(
        call(
            &f,
            "POST",
            &format!("{path}/preparation"),
            json!({"confirm":false,"expected_revision":revision})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, prepared) = call(
        &f,
        "POST",
        &format!("{path}/preparation"),
        json!({"confirm":true,"expected_revision":revision}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{prepared}");
    assert_eq!(prepared["status"], "ready");
    let (_, enabled) = call(&f, "GET", &path, Value::Null).await;
    assert_eq!(enabled["enabled"], true);
    assert_eq!(prepared["environment_ready"], true);
    assert_eq!(prepared["script_execution_ready"], false);
    assert_eq!(
        prepared["evidence"]["image_id"],
        f.sandbox.as_ref().unwrap().image_id()
    );
    let (_, runtime) = call(&f, "GET", &format!("{path}/runtime"), Value::Null).await;
    assert_eq!(runtime["applicable"], true);
    assert_eq!(runtime["ready"], true);
    assert_eq!(runtime["script_execution_ready"], false);
    f.sandbox.as_ref().unwrap().stop().await.unwrap();
}
#[tokio::test]
async fn original_directory_payload_preview_import_read_detail_update_and_delete() {
    let f = fixture().await;
    let input = package("Original 中文");
    let (status, preview) = call(&f, "POST", "/v1/imports/skill/preview", input.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["action"], "create");
    assert_eq!(
        call(&f, "GET", "/v1/capabilities", Value::Null).await.1,
        json!([])
    );
    let (status, record) = call(&f, "POST", "/v1/imports/skill/package", input).await;
    assert_eq!(status, StatusCode::OK, "{record}");
    let record = &record["capabilities"][0];
    let id = record["id"].as_str().unwrap();
    let revision = record["revision"].as_i64().unwrap();
    for endpoint in [
        "permissions",
        "health",
        "secrets",
        "runtime",
        "versions",
        "skill-files",
        "preparation",
    ] {
        assert_eq!(
            call(
                &f,
                "GET",
                &format!("/v1/capabilities/{id}/{endpoint}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::OK,
            "{endpoint}"
        );
    }
    let (_, instructions) = call(
        &f,
        "GET",
        &format!("/v1/capabilities/{id}/skill-instructions"),
        Value::Null,
    )
    .await;
    assert!(
        instructions["content"]
            .as_str()
            .unwrap()
            .contains("Original 中文")
    );
    assert_eq!(instructions["revision"], revision);
    let (_, enabled) = call(
        &f,
        "PUT",
        &format!("/v1/capabilities/{id}/enabled"),
        json!({"enabled":true,"expected_revision":revision,"actor":"spoofed"}),
    )
    .await;
    let revision = enabled["revision"].as_i64().unwrap();
    let (_, overview) = call(&f, "GET", "/v1/capabilities/overview", Value::Null).await;
    assert_eq!(overview[0]["availability"]["available"], true);
    assert_eq!(overview[0]["availability"]["script_execution_ready"], false);
    let mut next = package("Updated instructions");
    assert_eq!(
        call(&f, "POST", "/v1/imports/skill", next.clone()).await.0,
        StatusCode::CONFLICT
    );
    next["expected_revision"] = json!(revision);
    let (_, updated) = call(&f, "POST", "/v1/imports/skill", next).await;
    let revised = updated["capabilities"][0]["revision"].as_i64().unwrap();
    assert!(revised > revision);
    let (_, instructions) = call(
        &f,
        "GET",
        &format!("/v1/capabilities/{id}/skill-instructions"),
        Value::Null,
    )
    .await;
    assert!(
        instructions["content"]
            .as_str()
            .unwrap()
            .contains("Updated instructions")
    );
    assert_eq!(
        call(
            &f,
            "DELETE",
            &format!("/v1/capabilities/{id}"),
            json!({"expected_revision":revision})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &f,
            "DELETE",
            &format!("/v1/capabilities/{id}"),
            json!({"expected_revision":revised})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&f, "GET", &format!("/v1/capabilities/{id}"), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let store = f.registry.0.clone();
    let audit = tokio::task::spawn_blocking(move || {
        store
            .registry_audit(&RegistryAccess::Administrator, 100)
            .unwrap()
    })
    .await
    .unwrap();
    assert!(audit.iter().all(|event| event.actor == "admin"));
}
#[tokio::test]
async fn readonly_can_inspect_but_never_import_delete_prepare_or_enable() {
    let f = fixture().await;
    let (_, record) = call(&f, "POST", "/v1/imports/skill", package("read")).await;
    let id = record["capabilities"][0]["id"].as_str().unwrap();
    for endpoint in [
        "permissions",
        "health",
        "secrets",
        "runtime",
        "versions",
        "skill-instructions",
        "skill-files",
        "preparation",
    ] {
        assert_eq!(
            raw(
                &f.router,
                "GET",
                &format!("/v1/capabilities/{id}/{endpoint}"),
                Some(READONLY),
                String::new()
            )
            .await
            .0,
            StatusCode::OK,
            "{endpoint}"
        );
    }
    for (method, path) in [
        ("POST", "/v1/imports/skill".into()),
        ("DELETE", format!("/v1/capabilities/{id}")),
        ("POST", format!("/v1/capabilities/{id}/preparation")),
        ("PUT", format!("/v1/capabilities/{id}/enabled")),
    ] {
        assert_eq!(
            raw(
                &f.router,
                method,
                &path,
                Some(READONLY),
                package("deny").to_string()
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        raw(
            &f.router,
            "POST",
            "/v1/imports/skill",
            None,
            package("deny").to_string()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        raw(
            &f.router,
            "POST",
            "/v1/imports/skill",
            Some(USER),
            package("deny").to_string()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        raw(
            &f.portal,
            "POST",
            "/v1/imports/skill",
            Some(USER),
            package("deny").to_string()
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn malformed_native_json_has_clear_nonresumable_errors_and_does_not_register() {
    let f = fixture().await;
    for body in [
        r#"{"files":"[]"}"#,
        r#"{"files":[],"files":[]}"#,
        r#"{"files":[],"unknown":true}"#,
    ] {
        let (status, error) = raw(
            &f.router,
            "POST",
            "/v1/imports/skill",
            Some(ADMIN),
            body.into(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["resumable"], false);
        assert!(error["error"]["message"].as_str().unwrap().len() > 6);
    }
    assert_eq!(
        call(&f, "GET", "/v1/capabilities", Value::Null).await.1,
        json!([])
    );
}
