use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials, management_router};
use exoharness_configuration::Assistants;
use exoharness_storage_postgres::{
    CreateTurn, ExecutionSnapshot, ModelSnapshot, PostgresStore, RunState,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;
use tower::ServiceExt;

const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
async fn fixture() -> (Router, Router, PostgresStore) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_overview_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = tokio::task::spawn_blocking(move || {
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        let store = PostgresStore::connect(&url, &namespace).unwrap();
        store.migrate().unwrap();
        store
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
    let (_sender, stop) = watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store.clone())),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    (
        management_router(state.clone(), Arc::new(Assistants::empty())),
        management_router(
            ApiState {
                surface: Surface::Portal,
                port: 9798,
                ..state
            },
            Arc::new(Assistants::empty()),
        ),
        store,
    )
}
async fn get(router: &Router, port: u16, token: Option<&str>, path: &str) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .uri(path)
        .header("host", format!("127.0.0.1:{port}"));
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    if status == StatusCode::OK {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn empty_overview_is_successful_native_data_not_a_missing_route() {
    let (admin, portal, _store) = fixture().await;
    let (status, value) = get(&admin, 9797, Some(ADMIN), "/v1/management/overview").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["runs"], 0);
    assert_eq!(value["tokens"], 0);
    for pointer in [
        "/activity",
        "/models",
        "/assistants",
        "/groups/users",
        "/groups/assistants",
    ] {
        assert_eq!(value.pointer(pointer), Some(&json!([])));
    }
    assert_eq!(value["counts"], json!({}));
    assert_eq!(value["scheduler"]["ready"], true);
    assert_eq!(value["scheduler"]["global_limit"], 4);
    assert_eq!(value["scheduler"]["per_user_limit"], 2);
    assert_eq!(value["scheduler"]["active"], 0);
    assert_eq!(
        get(&admin, 9797, None, "/v1/management/overview").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(&admin, 9797, Some(USER), "/v1/management/overview")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&portal, 9798, Some(USER), "/v1/management/overview")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for path in [
        "/v1/management/overview?days=0",
        "/v1/management/overview?days=366",
        "/v1/management/overview?days=abc",
    ] {
        assert_eq!(
            get(&admin, 9797, Some(ADMIN), path).await.0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn overview_reads_real_committed_runs_and_applies_literal_user_filters() {
    let (admin, _portal, store) = fixture().await;
    tokio::task::spawn_blocking(move || {
        for (owner, id) in [("alice_1", "one"), ("bob", "two")] {
            store.create_session(owner, id).unwrap();
            let run = store
                .create_turn(CreateTurn {
                    owner,
                    session_id: id,
                    expected_session_version: 0,
                    turn_id: &format!("turn-{id}"),
                    run_id: &format!("run-{id}"),
                    input: &format!("task for {owner}"),
                    snapshot: &ExecutionSnapshot {
                        version: 1,
                        assistant_id: "assistant".into(),
                        assistant_revision: 1,
                        model: ModelSnapshot {
                            config_id: "model".into(),
                            revision: 1,
                            adapter: "mock".into(),
                            settings: json!({}),
                        },
                        capabilities: vec![],
                        max_consecutive_identical_calls: 3,
                        requires_execution: false,
                    },
                })
                .unwrap();
            if owner == "bob" {
                store
                    .transition_run(owner, id, &run.id, run.version, RunState::Cancelled, None)
                    .unwrap();
            }
        }
    })
    .await
    .unwrap();
    let (status, value) = get(&admin, 9797, Some(ADMIN), "/v1/management/overview?days=7").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["runs"], 2);
    assert_eq!(value["counts"], json!({"queued":1,"cancelled":1}));
    assert_eq!(value["activity"].as_array().unwrap().len(), 2);
    let (_, value) = get(
        &admin,
        9797,
        Some(ADMIN),
        "/v1/management/overview?user=ALICE_1",
    )
    .await;
    assert_eq!(value["runs"], 1);
    assert_eq!(value["activity"][0]["user_id"], "alice_1");
    let (_, value) = get(
        &admin,
        9797,
        Some(ADMIN),
        "/v1/management/overview?user=%25",
    )
    .await;
    assert_eq!(value["runs"], 0);
    assert_eq!(value["activity"], json!([]));
    let (_, value) = get(
        &admin,
        9797,
        Some(ADMIN),
        "/v1/management/overview?user=missing",
    )
    .await;
    assert_eq!(value["runs"], 0);
    assert_eq!(value["counts"], json!({}));
}
