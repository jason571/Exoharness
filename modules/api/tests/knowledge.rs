use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials};
use exoharness_configuration::Assistants;
use exoharness_sandbox::{Limits, RunSpec, Runtime, Sandbox};
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;
use tower::ServiceExt;

const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OTHER: &str = "cccccccccccccccccccccccccccccccc";
async fn fixture() -> (Router, Router) {
    fixture_with_sandbox(None).await
}
async fn fixture_with_sandbox(sandbox: Option<Arc<Sandbox>>) -> (Router, Router) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_knowledge_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = tokio::task::spawn_blocking(move || {
        let store = PostgresStore::connect(
            &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
                .expect("isolated PostgreSQL required"),
            &namespace,
        )
        .unwrap();
        store.migrate().unwrap();
        store.migrate().unwrap(); // rerunning additive migration retains all data
        store
    })
    .await
    .unwrap();
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"admin","role":"admin","token":ADMIN},
                {"id":"user","role":"user","token":USER},
                {"id":"other","role":"user","token":OTHER}
            ])
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
    (
        exoharness_api::knowledge_router_with_sandbox(
            state.clone(),
            Arc::new(Assistants::empty()),
            sandbox.clone(),
        ),
        exoharness_api::knowledge_router_with_sandbox(
            ApiState {
                surface: Surface::Portal,
                port: 9798,
                ..state
            },
            Arc::new(Assistants::empty()),
            sandbox,
        ),
    )
}

#[tokio::test]
async fn imported_documents_are_atomic_private_and_version_pinned() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let image = std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE")
        .expect("real prepared OCI image required");
    let sandbox = Arc::new(Sandbox::open(root.path().into(), &image).await.unwrap());
    let workspace = sandbox
        .create_workspace(
            "document-fixture",
            BTreeMap::from([(
                "fixtures.py".into(),
                include_bytes!("support/document_fixtures.py").to_vec(),
            )]),
        )
        .unwrap();
    let receipt = sandbox
        .start(RunSpec {
            invocation_id: "document-fixture".into(),
            workspace,
            read_only_package: None,
            runtime: Runtime::Python,
            entrypoint: "fixtures.py".into(),
            arguments: vec!["fixtures".into()],
            limits: Limits::default(),
        })
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(receipt.exit_code, Some(0), "{}", receipt.stderr);
    let fixtures: Value = serde_json::from_str(&receipt.stdout).unwrap();
    let (admin, portal) = fixture_with_sandbox(Some(sandbox.clone())).await;
    let body = json!({"filename":"销售资料.docx","file_base64":fixtures["docx"],"tags":[],"owner_user_id":"other"});
    let (status, manifest) = request(
        &portal,
        9798,
        Some(USER),
        "POST",
        "/v1/knowledge/documents/import",
        body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{manifest}");
    assert_eq!(manifest["blocks"][1]["row_count"], 200);
    assert!(!manifest.to_string().contains("/workspaces/"));
    assert!(manifest["blocks"][1].get("rows").is_none());
    let document = manifest["document_id"].as_str().unwrap();
    let rootpath = format!("/v1/knowledge/documents/{document}");
    let blockpath = format!("{rootpath}/blocks/table-1?revision=1&offset=2&limit=2");
    let (status, part) = request(&portal, 9798, Some(USER), "GET", &blockpath, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(part["block"]["partial"], true);
    assert_eq!(part["block"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(part["block"]["merged_anchors"][0]["text"], "农业");
    assert_eq!(part["block"]["headers"][0][0]["text"], "名称与金额（万元）");
    assert_eq!(
        request(&portal, 9798, Some(OTHER), "GET", &blockpath, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "GET",
            &format!("{rootpath}/blocks/table-1?revision=2"),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let hits = request(
        &portal,
        9798,
        Some(USER),
        "GET",
        "/v1/knowledge/search?q=200",
        Value::Null,
    )
    .await
    .1;
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert!(hits[0].get("blocks").is_none());
    // Failed parsing creates no partial document.
    let invalid = request(
        &portal,
        9798,
        Some(USER),
        "POST",
        "/v1/knowledge/documents/import",
        json!({"filename":"scan.pdf","file_base64":fixtures["blank_pdf"]}),
    )
    .await;
    assert_eq!(invalid.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid.1["error"]["code"], "pdf_without_text_layer");
    assert_eq!(invalid.1["resumable"], false);
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "GET",
            "/v1/knowledge/documents",
            Value::Null
        )
        .await
        .1
        .as_array()
        .unwrap()
        .len(),
        1
    );
    // Admin public-library import uses the identical parser, but portal cannot read it.
    let library = request(
        &admin,
        9797,
        Some(ADMIN),
        "POST",
        "/v1/knowledge/libraries",
        json!({"name":"PDF测试","description":"仅测试"}),
    )
    .await
    .1;
    let libraryroot = format!(
        "/v1/knowledge/libraries/{}",
        library["id"].as_str().unwrap()
    );
    let result = request(
        &admin,
        9797,
        Some(ADMIN),
        "POST",
        &format!("{libraryroot}/documents/import"),
        json!({"filename":"table.pdf","file_base64":fixtures["pdf"]}),
    )
    .await;
    assert_eq!(result.0, StatusCode::CREATED, "{}", result.1);
    assert_eq!(result.1["warnings"][0], "pdf_table_structure_unverified");
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "GET",
            &format!("{libraryroot}/documents"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    sandbox.stop().await.unwrap();
}

#[tokio::test]
async fn import_unavailable_is_not_resumable_or_a_fake_success() {
    let (_, portal) = fixture().await;
    let (status, value) = request(
        &portal,
        9798,
        Some(USER),
        "POST",
        "/v1/knowledge/documents/import",
        json!({"filename":"test.docx","file_base64":"YQ=="}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(value["error"]["code"], "document_parser_unavailable");
    assert_eq!(value["resumable"], false);
}
async fn request(
    router: &Router,
    port: u16,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", format!("127.0.0.1:{port}"))
        .header("content-type", "application/json");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn personal_documents_categories_and_memories_match_original_page_contracts() {
    let (_, portal) = fixture().await;
    for path in [
        "/v1/knowledge/categories",
        "/v1/knowledge/documents",
        "/v1/memories",
    ] {
        assert_eq!(
            request(&portal, 9798, Some(USER), "GET", path, Value::Null).await,
            (StatusCode::OK, json!([]))
        );
        assert_eq!(
            request(&portal, 9798, None, "GET", path, Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, category) = request(
        &portal,
        9798,
        Some(USER),
        "POST",
        "/v1/knowledge/categories",
        json!({"name":"学习","parent_id":null,"expected_revision":null}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(category["path"], "学习");
    let id = category["id"].as_str().unwrap();
    let (status,document)=request(&portal,9798,Some(USER),"POST","/v1/knowledge/documents",json!({"title":"太阳系","content":"水星、金星、地球。","category_id":id,"tags":["科学"],"source_uri":null,"expected_revision":null})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(document["category_path"], "学习");
    assert_eq!(document["sha256"].as_str().unwrap().len(), 64);
    let path = format!(
        "/v1/knowledge/documents/{}",
        document["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&portal, 9798, Some(USER), "GET", &path, Value::Null)
            .await
            .1,
        document
    );
    assert_eq!(
        request(&portal, 9798, Some(OTHER), "GET", &path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&portal, 9798, Some(OTHER), "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let hits = request(
        &portal,
        9798,
        Some(USER),
        "GET",
        "/v1/knowledge/search?q=%E5%9C%B0%E7%90%83",
        Value::Null,
    )
    .await
    .1;
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert!(hits[0].get("content").is_none());
    assert_eq!(
        request(
            &portal,
            9798,
            Some(OTHER),
            "GET",
            "/v1/knowledge/search?q=%E5%9C%B0%E7%90%83",
            Value::Null
        )
        .await
        .1,
        json!([])
    );
    let edited =
        json!({"title":"改名","content":"新资料","category_id":id,"tags":[],"expected_revision":1});
    assert_eq!(
        request(&portal, 9798, Some(USER), "PUT", &path, edited.clone())
            .await
            .1["revision"],
        2
    );
    assert_eq!(
        request(&portal, 9798, Some(USER), "PUT", &path, edited)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "DELETE",
            &format!("/v1/knowledge/categories/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let document = request(&portal, 9798, Some(USER), "GET", &path, Value::Null)
        .await
        .1;
    assert!(document["category_id"].is_null());
    assert_eq!(document["revision"], 3);
    let (status, memory) = request(
        &portal,
        9798,
        Some(USER),
        "POST",
        "/v1/memories",
        json!({"content":"用中文回答","enabled":true,"expected_revision":null}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(memory["source"], "user");
    let path = format!("/v1/memories/{}", memory["id"].as_str().unwrap());
    assert_eq!(
        request(
            &portal,
            9798,
            Some(OTHER),
            "PUT",
            &path,
            json!({"content":"改变他人数据","enabled":false,"expected_revision":1})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "PUT",
            &path,
            json!({"content":"用中文回答","enabled":false,"expected_revision":1})
        )
        .await
        .1["revision"],
        2
    );
    assert_eq!(
        request(&portal, 9798, Some(USER), "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn hierarchy_checks_depth_cycles_subtrees_and_cross_user_parent() {
    let (_, portal) = fixture().await;
    let mut parent = Value::Null;
    let mut ids = Vec::new();
    for name in ["一级", "二级", "三级"] {
        let (status, value) = request(
            &portal,
            9798,
            Some(USER),
            "POST",
            "/v1/knowledge/categories",
            json!({"name":name,"parent_id":parent}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        ids.push(value["id"].as_str().unwrap().to_owned());
        parent = value["id"].clone();
    }
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "POST",
            "/v1/knowledge/categories",
            json!({"name":"四级","parent_id":parent})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(OTHER),
            "POST",
            "/v1/knowledge/categories",
            json!({"name":"错误归类","parent_id":ids[0]})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let path = format!("/v1/knowledge/categories/{}", ids[0]);
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "PUT",
            &path,
            json!({"name":"循环","parent_id":ids[2],"expected_revision":1})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&portal, 9798, Some(USER), "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "PUT",
            &path,
            json!({"name":"新一级","parent_id":null,"expected_revision":1})
        )
        .await
        .0,
        StatusCode::OK
    );
    let records = request(
        &portal,
        9798,
        Some(USER),
        "GET",
        "/v1/knowledge/categories",
        Value::Null,
    )
    .await
    .1;
    for record in records.as_array().unwrap() {
        assert!(record["path"].as_str().unwrap().starts_with("新一级"));
        assert_eq!(record["revision"], 2);
    }
}

#[tokio::test]
async fn library_routes_are_admin_only_and_delete_all_owned_content() {
    let (admin, portal) = fixture().await;
    assert_eq!(
        request(
            &admin,
            9797,
            Some(USER),
            "GET",
            "/v1/knowledge/libraries",
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &portal,
            9798,
            Some(USER),
            "GET",
            "/v1/knowledge/libraries",
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, library) = request(
        &admin,
        9797,
        Some(ADMIN),
        "POST",
        "/v1/knowledge/libraries",
        json!({"name":"公司资料","description":"共享资料"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let root = format!(
        "/v1/knowledge/libraries/{}",
        library["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&admin, 9797, Some(ADMIN), "GET", &root, Value::Null)
            .await
            .1,
        library
    );
    assert_eq!(
        request(
            &admin,
            9797,
            Some(ADMIN),
            "PUT",
            &format!("{root}/grants"),
            json!({"assistant_ids":["nonexistent"]})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &admin,
            9797,
            Some(ADMIN),
            "PUT",
            &format!("{root}/grants"),
            json!({"assistant_ids":[]})
        )
        .await
        .1["revision"],
        2
    );
    let category = request(
        &admin,
        9797,
        Some(ADMIN),
        "POST",
        &format!("{root}/categories"),
        json!({"name":"部门","parent_id":null}),
    )
    .await
    .1;
    assert_eq!(request(&admin,9797,Some(ADMIN),"POST",&format!("{root}/documents"),json!({"title":"业务介绍","content":"保留准确事实","tags":[],"category_id":category["id"]})).await.0,StatusCode::CREATED);
    assert_eq!(
        request(&admin, 9797, Some(ADMIN), "DELETE", &root, Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &admin,
            9797,
            Some(ADMIN),
            "GET",
            &format!("{root}/documents"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
