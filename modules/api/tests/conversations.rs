use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials, conversation_router};
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

const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OTHER: &str = "cccccccccccccccccccccccccccccccc";
async fn fixture() -> (Router, PostgresStore) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_conversation_{}_{}",
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
        store
    })
    .await
    .unwrap();
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"user","role":"user","token":USER},
                {"id":"other","role":"user","token":OTHER}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let (_, stop) = watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store.clone())),
        credentials,
        surface: Surface::Portal,
        port: 9798,
        stop,
    };
    (
        conversation_router(state, Arc::new(Assistants::empty())),
        store,
    )
}
async fn request(
    router: &Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:9798")
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
async fn original_session_list_search_pin_rename_detail_and_delete_are_scoped() {
    let (router, store) = fixture().await;
    assert_eq!(
        request(&router, None, "GET", "/v1/sessions", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&router, Some(USER), "GET", "/v1/sessions", Value::Null).await,
        (StatusCode::OK, json!([]))
    );
    let (status, session) = request(
        &router,
        Some(USER),
        "POST",
        "/v1/sessions",
        json!({"title":null,"actor":"other"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(session["title"], "新对话");
    assert_eq!(session["turn_count"], 0);
    assert_eq!(session["pinned"], false);
    let id = session["id"].as_str().unwrap().to_owned();
    let path = format!("/v1/sessions/{id}");
    assert_eq!(
        request(&router, Some(OTHER), "GET", &path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&router, Some(OTHER), "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let value = request(
        &router,
        Some(USER),
        "PATCH",
        &path,
        json!({"title":"science","pinned":true}),
    )
    .await
    .1;
    assert_eq!(value["pinned"], true);
    assert_eq!(
        request(
            &router,
            Some(USER),
            "GET",
            "/v1/sessions?q=science",
            Value::Null
        )
        .await
        .1
        .as_array()
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "GET",
            "/v1/sessions?q=missing",
            Value::Null
        )
        .await
        .1,
        json!([])
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "PATCH",
            &path,
            json!({"owner_user_id":"other"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let detail = request(&router, Some(USER), "GET", &path, Value::Null)
        .await
        .1;
    assert_eq!(detail["turns"], json!([]));
    assert_eq!(detail["notices"], json!([]));
    assert_eq!(detail["history_page"]["has_more"], false);
    assert_eq!(
        request(
            &router,
            Some(USER),
            "GET",
            &format!("{path}/response-actions"),
            Value::Null
        )
        .await
        .1,
        json!({})
    );
    assert_eq!(
        request(&router, Some(USER), "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&router, Some(USER), "GET", &path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // Evidence is not destroyed by sidebar deletion.
    tokio::task::spawn_blocking(move || assert!(store.session_version("user", &id).is_err()))
        .await
        .unwrap();
}
#[tokio::test]
async fn history_uses_ordinal_pages_and_feedback_event_ids_are_idempotent() {
    let (router, store) = fixture().await;
    tokio::task::spawn_blocking(move || {
        store
            .new_conversation("user", "history", "history", &json!({"id":"test-assistant","display_name":"测试助理","system_prompt":"private","access":{"all":true}}))
            .unwrap();
        for n in 1..=4 {
            let version = store.session_version("user", "history").unwrap();
            let run_id = format!("run-{n}");
            let record = store
                .create_turn(CreateTurn {
                    owner: "user",
                    session_id: "history",
                    expected_session_version: version,
                    turn_id: &format!("turn-{n}"),
                    run_id: &run_id,
                    input: "question",
                    snapshot: &ExecutionSnapshot {
                        version: 1,
                        assistant_id: "test-assistant".into(),
                        assistant_revision: 1,
                        model: ModelSnapshot {
                            config_id: "test-model".into(),
                            revision: 1,
                            adapter: "openai".into(),
                            settings: json!({}),
                        },
                        capabilities: vec![],
                        max_consecutive_identical_calls: 3,
                        requires_execution: false,
                    },
                })
                .unwrap();
            let record = store
                .transition_run(
                    "user",
                    "history",
                    &run_id,
                    record.version,
                    RunState::Running,
                    None,
                )
                .unwrap();
            store
                .transition_run(
                    "user",
                    "history",
                    &run_id,
                    record.version,
                    RunState::Succeeded,
                    None,
                )
                .unwrap();
        }
    })
    .await
    .unwrap();
    let page = request(
        &router,
        Some(USER),
        "GET",
        "/v1/sessions/history?turn_limit=2",
        Value::Null,
    )
    .await
    .1;
    assert_eq!(page["turn_count"], 4);
    assert_eq!(page["agent_profile"]["display_name"], "测试助理");
    assert!(page["agent_profile"].get("system_prompt").is_none());
    assert!(page["agent_profile"].get("access").is_none());
    assert_eq!(page["turns"][0]["ordinal"], 3);
    assert_eq!(page["turns"][1]["ordinal"], 4);
    assert_eq!(page["history_page"]["has_more"], true);
    assert_eq!(page["history_page"]["before_ordinal"], 3);
    let older = request(
        &router,
        Some(USER),
        "GET",
        "/v1/sessions/history?turn_limit=2&before_ordinal=3",
        Value::Null,
    )
    .await
    .1;
    assert_eq!(older["turns"][0]["ordinal"], 1);
    assert_eq!(older["history_page"]["has_more"], false);
    let path = "/v1/runs/run-4/response-actions";
    let body =
        json!({"event_id":"event-one","action":"dislike","reason":"other","detail":"需要改进"});
    let saved = request(&router, Some(USER), "POST", path, body.clone()).await;
    assert_eq!(saved.0, StatusCode::OK);
    assert_eq!(saved.1["state"]["feedback"], "dislike");
    assert_eq!(
        request(&router, Some(USER), "POST", path, body).await,
        saved
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "POST",
            path,
            json!({"event_id":"event-one","action":"like"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&router, Some(OTHER), "GET", path, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "POST",
            path,
            json!({"event_id":"too-long","action":"dislike","detail":"字".repeat(201)})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "POST",
            path,
            json!({"event_id":"clear","action":"clear_feedback"})
        )
        .await
        .1["state"]["feedback"],
        Value::Null
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "POST",
            path,
            json!({"event_id":"regen","action":"regenerate"})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        request(
            &router,
            Some(USER),
            "GET",
            "/v1/sessions/history/response-actions?turn_limit=2",
            Value::Null
        )
        .await
        .1
        .as_object()
        .unwrap()
        .len(),
        2
    );
}
