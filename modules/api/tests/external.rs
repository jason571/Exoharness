#[path = "../../model-runtime/tests/support/server.rs"]
#[allow(dead_code)]
mod server;
#[path = "support/identity.rs"]
#[allow(dead_code)]
mod support;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{ApiState, ChatTasks, Surface, conversation_router, external_router};
use exoharness_configuration::Models;
use exoharness_identity::external::pkce_challenge;
use exoharness_model_runtime::{ModelRuntime, RuntimePolicy};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ1234567890";
#[allow(clippy::too_many_arguments)]
async fn direct(
    router: &Router,
    app: &Value,
    user: &str,
    device: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:9798")
        .header("content-type", "application/json")
        .header(
            "authorization",
            format!("Bearer {}", app["client_secret"].as_str().unwrap()),
        )
        .header(
            "x-exoharness-client-id",
            app["application"]["client_id"].as_str().unwrap(),
        )
        .header("x-exoharness-user-id", user)
        .header(
            "x-exoharness-user-name",
            if user == "local_one" {
                "flyang"
            } else {
                "Second"
            },
        );
    if let Some(device) = device {
        request = request.header("x-exoharness-device-id", device);
    }
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create_direct_app(f: &support::Fixture, id: &str, required: bool) -> Value {
    let (cookie, csrf) = support::admin_login(f).await;
    let mut input = application(id, "coder");
    input["require_device_id"] = json!(required);
    input["redirect_uris"] = json!([]);
    let response = support::request(
        &f.admin,
        9797,
        "POST",
        "/v1/management/external-applications",
        input,
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED);
    response.body
}

#[tokio::test]
async fn application_key_direct_calls_keep_users_devices_history_and_admin_search_isolated() {
    let f = support::fixture().await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let app = create_direct_app(&f, "care", true).await;
    let mut sessions = vec![];
    for (user, device) in [
        ("local_one", "watch-a"),
        ("local_one", "watch-b"),
        ("local_two", "watch-a"),
    ] {
        let (status, result) = direct(
            &router,
            &app,
            user,
            Some(device),
            "POST",
            "/v1/external/sessions",
            json!({"title":device,"device_id":device}),
            Some(device),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        sessions.push(result);
    }
    assert_ne!(sessions[0]["id"], sessions[1]["id"]);
    assert_ne!(sessions[0]["id"], sessions[2]["id"]);
    let (_, me) = direct(
        &router,
        &app,
        "local_one",
        Some("watch-a"),
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    let (_, other) = direct(
        &router,
        &app,
        "local_two",
        Some("watch-a"),
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    assert_ne!(me["account"]["id"], other["account"]["id"]);
    let sid = sessions[0]["id"].as_str().unwrap();
    assert_eq!(
        direct(
            &router,
            &app,
            "local_one",
            Some("watch-b"),
            "GET",
            &format!("/v1/external/sessions/{sid}"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        direct(
            &router,
            &app,
            "local_two",
            Some("watch-a"),
            "GET",
            &format!("/v1/external/sessions/{sid}"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, run) = direct(
        &router,
        &app,
        "local_one",
        Some("watch-a"),
        "POST",
        &format!("/v1/external/sessions/{sid}/turns"),
        json!({"prompt":"Private app-key conversation"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    for _ in 0..100 {
        let store = f.admin_state.execution.0.clone();
        let owner = me["account"]["id"].as_str().unwrap().to_owned();
        let run = run["run_id"].as_str().unwrap().to_owned();
        if tokio::task::spawn_blocking(move || {
            store.owned_run(&owner, &run).unwrap().state
                == exoharness_storage_postgres::RunState::Succeeded
        })
        .await
        .unwrap()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let (admin, csrf) = support::admin_login(&f).await;
    let list = support::request(
        &f.admin,
        9797,
        "GET",
        "/v1/management/external-conversations?user=flyang",
        Value::Null,
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.body["total"], 2);
    let list = support::request(
        &f.admin,
        9797,
        "GET",
        "/v1/management/external-conversations?q=local_two",
        Value::Null,
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(list.body["total"], 1);
    assert_eq!(list.body["sessions"][0]["external_user_id"], "local_two");
    let detail = support::request(
        &f.admin,
        9797,
        "GET",
        &format!("/v1/management/external-applications/care/sessions/{sid}"),
        Value::Null,
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(detail.body["turns"][0]["answer"], "OK");
    assert_eq!(detail.body["external"]["turn_count"], 1);
    assert_eq!(detail.body["external"]["device_id"], "watch-a");
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "GET",
            "/v1/management/external-conversations?limit=11",
            Value::Null,
            Some(&admin),
            None
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "GET",
            "/v1/management/external-conversations",
            Value::Null,
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "GET",
            &format!("/v1/management/external-applications/other/sessions/{sid}"),
            Value::Null,
            Some(&admin),
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert!(!csrf.is_empty());
    assert_eq!(upstream.count(), 1);
    tasks.close().await;
}

#[tokio::test]
async fn device_requirement_applies_only_to_configured_apps_and_legacy_unbound_turns_never_execute()
{
    let f = support::fixture().await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let app = create_direct_app(&f, "care", true).await;
    let other = create_direct_app(&f, "plain", false).await;
    assert_eq!(
        direct(
            &router,
            &app,
            "local_one",
            None,
            "POST",
            "/v1/external/sessions",
            json!({}),
            Some("missing")
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        direct(
            &router,
            &other,
            "local_one",
            None,
            "POST",
            "/v1/external/sessions",
            json!({}),
            Some("ordinary-app")
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (_, me) = direct(
        &router,
        &app,
        "local_one",
        Some("watch"),
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    let store = f.admin_state.execution.0.clone();
    let user = me["account"]["id"].as_str().unwrap().to_owned();
    let old = tokio::task::spawn_blocking(move || {
        store
            .create_external_session(exoharness_storage_postgres::ExternalSessionRequest {
                owner: &user,
                client_id: "care",
                request_key: "legacy",
                title: Some("Old"),
                profile: &json!({"id":"coder"}),
            })
            .unwrap()
    })
    .await
    .unwrap();
    let id = old["id"].as_str().unwrap();
    assert_eq!(
        direct(
            &router,
            &app,
            "local_one",
            Some("watch"),
            "POST",
            &format!("/v1/external/sessions/{id}/turns"),
            json!({"prompt":"Must not execute"}),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        direct(
            &router,
            &app,
            "local_one",
            Some("watch"),
            "PUT",
            &format!("/v1/external/sessions/{id}/device"),
            json!({"device_id":"watch"}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        direct(
            &router,
            &app,
            "local_one",
            Some("watch"),
            "PUT",
            &format!("/v1/external/sessions/{id}/device"),
            json!({"device_id":"other"}),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let(status,token)=raw(&router,"","POST","/v1/external/token",json!({"client_id":"care","client_secret":app["client_secret"],"grant_type":"app_user","external_user_id":"local_one","display_name":"flyang","device_id":"watch"}),None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(token["access_token"].as_str().is_some());
    assert_eq!(upstream.count(), 0);
    tasks.close().await;
}
async fn external(
    f: &support::Fixture,
    upstream: &server::Server,
) -> (Router, Arc<ChatTasks>, tokio::sync::watch::Sender<bool>) {
    let root = f._root.path().join("models");
    let url = upstream.url.clone();
    let models=tokio::task::spawn_blocking(move||{let m=Models::initialize(&root).unwrap();m.save_configuration(json!({"name":"fixture","api":"openai_compatible","base_url":url,"enabled":true}),json!({"name":"llm","provider":"fixture","model_id":"test-model","display_name":"Mock","enabled":true,"max_tokens":128,"thinking":"none"}),Some("fixture-only-private-key"),None).unwrap();m.set_default("llm",None).unwrap();Arc::new(m)}).await.unwrap();
    let runtime = Arc::new(ModelRuntime::new(models.clone(), RuntimePolicy::default()).unwrap());
    let tasks = Arc::new(ChatTasks::default());
    let (stop, rx) = tokio::sync::watch::channel(false);
    let state = ApiState {
        surface: Surface::Portal,
        port: 9798,
        stop: rx,
        ..f.admin_state.clone()
    };
    let router = f
        .portal
        .clone()
        .merge(conversation_router(state.clone(), f.assistants.clone()))
        .merge(external_router(
            state,
            models,
            f.assistants.clone(),
            runtime,
            tasks.clone(),
            None,
        ));
    (router, tasks, stop)
}
fn application(id: &str, role: &str) -> Value {
    json!({"client_id":id,"name":format!("App {id}"),"assistant_id":role,"redirect_uris":["https://app.example/callback"],"scopes":["identity:read","sessions:read","sessions:write","runs:read","runs:cancel"]})
}
async fn raw(
    router: &Router,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:9798")
        .header("content-type", "application/json");
    if !token.is_empty() {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if let Some(key) = key {
        builder = builder.header("idempotency-key", key);
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn create_app(f: &support::Fixture, id: &str, role: &str) -> Value {
    let (cookie, csrf) = support::admin_login(f).await;
    let reply = support::request(
        &f.admin,
        9797,
        "POST",
        "/v1/management/external-applications",
        application(id, role),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    assert!(
        reply.body["client_secret"]
            .as_str()
            .unwrap()
            .starts_with("exo_")
    );
    reply.body
}
async fn authorize(router: &Router, cookie: &str, csrf: &str, app: &Value, scope: &str) -> Value {
    let request = json!({"client_id":app["application"]["client_id"],"redirect_uri":"https://app.example/callback","scope":scope,"state":"random-state-1234567890","code_challenge":pkce_challenge(VERIFIER).unwrap(),"code_challenge_method":"S256","response_type":"code"});
    let response = support::request(
        router,
        9798,
        "POST",
        "/v1/external/authorize",
        json!({"request":request,"confirm":true}),
        Some(cookie),
        Some(csrf),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    let url = url::Url::parse(response.body["redirect_uri"].as_str().unwrap()).unwrap();
    let code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let response=support::request(router,9798,"POST","/v1/external/token",json!({"client_id":app["application"]["client_id"],"client_secret":app["client_secret"],"grant_type":"authorization_code","code":code,"redirect_uri":"https://app.example/callback","code_verifier":VERIFIER}),None,None).await;
    assert_eq!(response.status, StatusCode::OK);
    response.body
}
const SCOPES: &str = "identity:read sessions:read sessions:write runs:read runs:cancel";
#[tokio::test]
async fn revoked_grant_cannot_start_a_queued_run_or_spend_another_model_request() {
    let f = support::fixture().await;
    let (user, cookie, csrf) = support::register(&f).await;
    let mut reply = server::Reply::json(server::answer());
    reply.delay = Duration::from_secs(2);
    let upstream = server::Server::new(reply).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let store = f.admin_state.execution.0.clone();
    tokio::task::spawn_blocking(move || {
        let mut config = store.scheduler_configuration().unwrap();
        config.max_active_runs_per_user = 1;
        store.save_scheduler_configuration(config).unwrap();
    })
    .await
    .unwrap();
    let app = create_app(&f, "queued", "coder").await;
    let tokens = authorize(&router, &cookie, &csrf, &app, SCOPES).await;
    let token = tokens["access_token"].as_str().unwrap();
    let (_, a) = raw(
        &router,
        token,
        "POST",
        "/v1/external/sessions",
        json!({"title":"A"}),
        Some("queue-a"),
    )
    .await;
    let (_, b) = raw(
        &router,
        token,
        "POST",
        "/v1/external/sessions",
        json!({"title":"B"}),
        Some("queue-b"),
    )
    .await;
    let (_, first) = raw(
        &router,
        token,
        "POST",
        &format!("/v1/external/sessions/{}/turns", a["id"].as_str().unwrap()),
        json!({"prompt":"First request"}),
        None,
    )
    .await;
    for _ in 0..100 {
        if upstream.count() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(upstream.count(), 1);
    let (status, second) = raw(
        &router,
        token,
        "POST",
        &format!("/v1/external/sessions/{}/turns", b["id"].as_str().unwrap()),
        json!({"prompt":"Must remain unsubmitted after revocation"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (store, owner, grant) = (
        f.store.clone(),
        user.clone(),
        tokens["grant_id"].as_str().unwrap().to_owned(),
    );
    assert!(
        tokio::task::spawn_blocking(move || store.revoke_external_grant(&owner, &grant).unwrap())
            .await
            .unwrap()
    );
    assert_eq!(
        raw(&router, token, "GET", "/v1/external/me", Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let first_id = first["run_id"].as_str().unwrap().to_owned();
    let second_id = second["run_id"].as_str().unwrap().to_owned();
    let mut final_states = None;
    for _ in 0..150 {
        let (store, owner, one, two) = (
            f.admin_state.execution.0.clone(),
            user.clone(),
            first_id.clone(),
            second_id.clone(),
        );
        let states = tokio::task::spawn_blocking(move || {
            (
                store.owned_run(&owner, &one).unwrap().state,
                store.owned_run(&owner, &two).unwrap().state,
            )
        })
        .await
        .unwrap();
        if !states.0.active() && !states.1.active() {
            final_states = Some(states);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        final_states,
        Some((
            exoharness_storage_postgres::RunState::Succeeded,
            exoharness_storage_postgres::RunState::Failed
        ))
    );
    assert_eq!(upstream.count(), 1);
    tasks.close().await;
}
#[tokio::test]
async fn two_devices_use_distinct_sessions_under_one_user_and_applications_are_isolated() {
    let f = support::fixture().await;
    let (user, cookie, csrf) = support::register(&f).await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let a = create_app(&f, "app_a", "coder").await;
    let b = create_app(&f, "app_b", "xiaolan").await;
    let ta = authorize(&router, &cookie, &csrf, &a, SCOPES).await;
    let tb = authorize(&router, &cookie, &csrf, &b, SCOPES).await;
    let token = ta["access_token"].as_str().unwrap();
    let token_b = tb["access_token"].as_str().unwrap();
    let (status, me) = raw(&router, token, "GET", "/v1/external/me", Value::Null, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["account"]["id"], user);
    assert_eq!(me["assistant_id"], "coder");
    let (status, sa) = raw(
        &router,
        token,
        "POST",
        "/v1/external/sessions",
        json!({"title":"Watch A"}),
        Some("conversation-a"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, sb) = raw(
        &router,
        token,
        "POST",
        "/v1/external/sessions",
        json!({"title":"Watch B"}),
        Some("conversation-b"),
    )
    .await;
    assert_ne!(sa["id"], sb["id"]);
    assert_eq!(sa["agent_profile"]["id"], "coder");
    let (_, replay) = raw(
        &router,
        token,
        "POST",
        "/v1/external/sessions",
        json!({"title":"Watch A"}),
        Some("conversation-a"),
    )
    .await;
    assert_eq!(sa["id"], replay["id"]);
    assert_eq!(
        raw(
            &router,
            token,
            "POST",
            "/v1/external/sessions",
            json!({"title":"Changed"}),
            Some("conversation-a")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let sid = sa["id"].as_str().unwrap();
    assert_eq!(
        raw(
            &router,
            token_b,
            "GET",
            &format!("/v1/external/sessions/{sid}"),
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        raw(&router, token, "GET", "/v1/account", Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        raw(
            &router,
            a["client_secret"].as_str().unwrap(),
            "GET",
            "/v1/external/me",
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let prompts = [
        ("Watch A private prompt", sa["id"].as_str().unwrap()),
        ("Watch B independent prompt", sb["id"].as_str().unwrap()),
    ];
    let mut runs = Vec::new();
    for (prompt, sid) in prompts {
        let (status, run) = raw(
            &router,
            token,
            "POST",
            &format!("/v1/external/sessions/{sid}/turns"),
            json!({"prompt":prompt}),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        runs.push(run["run_id"].as_str().unwrap().to_owned());
    }
    for _ in 0..100 {
        let store = f.admin_state.execution.0.clone();
        let owner = user.clone();
        let ids = runs.clone();
        let done = tokio::task::spawn_blocking(move || {
            ids.iter().all(|id| {
                store.owned_run(&owner, id).unwrap().state
                    == exoharness_storage_postgres::RunState::Succeeded
            })
        })
        .await
        .unwrap();
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(upstream.count(), 2);
    for index in 0..2 {
        let (_, history) = raw(
            &router,
            token,
            "GET",
            &format!("/v1/external/sessions/{}", prompts[index].1),
            Value::Null,
            None,
        )
        .await;
        assert_eq!(history["turns"][0]["prompt"], prompts[index].0);
        assert_eq!(history["turns"][0]["answer"], "OK");
        let store = f.admin_state.execution.0.clone();
        let owner = user.clone();
        let id = runs[index].clone();
        let record = tokio::task::spawn_blocking(move || store.owned_run(&owner, &id).unwrap())
            .await
            .unwrap();
        assert_eq!(record.snapshot.assistant_id, "coder");
        assert_eq!(
            record.state,
            exoharness_storage_postgres::RunState::Succeeded
        );
        assert_eq!(
            record.snapshot.model.settings["external_application"]["client_id"],
            "app_a"
        );
    }
    {
        let requests = upstream.state.received.lock().unwrap();
        for (_, request) in requests.iter() {
            let content = request.to_string();
            assert!(!(content.contains(prompts[0].0) && content.contains(prompts[1].0)));
        }
    }
    assert_eq!(
        raw(
            &router,
            token_b,
            "POST",
            &format!("/v1/external/runs/{}/cancel", runs[0]),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    tasks.close().await;
    let store = f.store.clone();
    let owner = user.clone();
    assert_eq!(
        tokio::task::spawn_blocking(move || store.user_agent_profile_id(&owner).unwrap())
            .await
            .unwrap()
            .as_deref(),
        Some("xiaolan")
    );
}
#[tokio::test]
async fn delegated_scope_and_identity_cannot_be_selected_by_request_body() {
    let f = support::fixture().await;
    let (_, cookie, csrf) = support::register(&f).await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, _, _stop) = external(&f, &upstream).await;
    let app = create_app(&f, "limited", "coder").await;
    let tokens = authorize(&router, &cookie, &csrf, &app, "identity:read").await;
    let token = tokens["access_token"].as_str().unwrap();
    assert_eq!(
        raw(
            &router,
            token,
            "POST",
            "/v1/external/sessions",
            json!({}),
            Some("new")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let tokens = authorize(&router, &cookie, &csrf, &app, SCOPES).await;
    let token = tokens["access_token"].as_str().unwrap();
    assert_eq!(
        raw(
            &router,
            token,
            "POST",
            "/v1/external/sessions",
            json!({"user_id":"other","assistant_id":"xiaolan"}),
            Some("new")
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        raw(
            &router,
            token,
            "POST",
            "/v1/external/sessions",
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (cookie_admin, csrf_admin) = support::admin_login(&f).await;
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "POST",
            "/v1/management/external-applications",
            application("bad", "inactive"),
            Some(&cookie_admin),
            Some(&csrf_admin)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        support::request(
            &router,
            9798,
            "GET",
            "/v1/external/me",
            Value::Null,
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(upstream.count(), 0);
}
#[tokio::test]
async fn generated_keys_are_encrypted_viewable_by_admin_and_rotation_does_not_affect_other_apps() {
    let f = support::fixture().await;
    let (cookie, csrf) = support::admin_login(&f).await;
    let a = create_app(&f, "key_a", "coder").await;
    let b = create_app(&f, "key_b", "xiaolan").await;
    let path = "/v1/management/external-applications/key_a/key";
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "POST",
            path,
            json!({"confirm":true}),
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "POST",
            path,
            json!({"confirm":false}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    let view = support::request(
        &f.admin,
        9797,
        "POST",
        path,
        json!({"confirm":true}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.body["client_secret"], a["client_secret"]);
    let list = support::request(
        &f.admin,
        9797,
        "GET",
        "/v1/management/external-applications",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert!(
        !list
            .body
            .to_string()
            .contains(a["client_secret"].as_str().unwrap())
    );
    let store = f.store.clone();
    let encrypted = tokio::task::spawn_blocking(move || {
        store.external_encrypted_key("key_a").unwrap().unwrap()
    })
    .await
    .unwrap();
    assert!(!encrypted.contains(a["client_secret"].as_str().unwrap()));
    assert!(encrypted.starts_with("v1."));
    let rotated=support::request(&f.admin,9797,"PUT","/v1/management/external-applications/key_a",json!({"application":application("key_a","coder"),"expected_revision":1,"rotate_secret":true}),Some(&cookie),Some(&csrf)).await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert!(
        rotated.body["client_secret"]
            .as_str()
            .unwrap()
            .starts_with("exo_")
    );
    assert_ne!(rotated.body["client_secret"], a["client_secret"]);
    let view = support::request(
        &f.admin,
        9797,
        "POST",
        path,
        json!({"confirm":true}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(view.body["client_secret"], rotated.body["client_secret"]);
    let store = f.store.clone();
    let other = b["client_secret"].as_str().unwrap().to_owned();
    assert!(
        tokio::task::spawn_blocking(move || store
            .authenticate_external_application("key_b", &other)
            .unwrap()
            .is_some())
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn private_assistant_can_run_for_assigned_application_and_deletion_preserves_history() {
    let f = support::fixture().await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let app = create_app(&f, "private-app", "private").await;
    assert!(f.assistants.public_profile("private").is_null());
    assert!(f.assistants.assignment("private").is_err());
    assert_eq!(
        f.assistants.detail("private").unwrap()["access"]["published"],
        false
    );
    let (status, session) = direct(
        &router,
        &app,
        "registered-user",
        Some("watch-a"),
        "POST",
        "/v1/external/sessions",
        json!({"title":"Private app history","device_id":"watch-a"}),
        Some("conversation"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(session["agent_profile"]["id"], "private");
    assert!(session["agent_profile"].get("system_prompt").is_none());
    let sid = session["id"].as_str().unwrap();
    let (status, turn) = direct(
        &router,
        &app,
        "registered-user",
        Some("watch-a"),
        "POST",
        &format!("/v1/external/sessions/{sid}/turns"),
        json!({"prompt":"A preserved request"}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, me) = direct(
        &router,
        &app,
        "registered-user",
        Some("watch-a"),
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    let owner = me["account"]["id"].as_str().unwrap().to_owned();
    let run = turn["run_id"].as_str().unwrap().to_owned();
    let mut finished = false;
    for _ in 0..100 {
        let (store, user, id) = (
            f.admin_state.execution.0.clone(),
            owner.clone(),
            run.clone(),
        );
        let state = tokio::task::spawn_blocking(move || store.owned_run(&user, &id).unwrap().state)
            .await
            .unwrap();
        if state == exoharness_storage_postgres::RunState::Succeeded {
            finished = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(finished);
    assert_eq!(upstream.count(), 1);
    let (cookie, csrf) = support::admin_login(&f).await;
    let path = "/v1/management/external-applications/private-app";
    let deletion = json!({"expected_revision":1});
    let forbidden_request = Request::builder()
        .method("DELETE")
        .uri(path)
        .header("host", "127.0.0.1:9797")
        .header("content-type", "application/json")
        .header(
            "authorization",
            format!("Bearer {}", app["client_secret"].as_str().unwrap()),
        )
        .body(Body::from(deletion.to_string()))
        .unwrap();
    assert_eq!(
        f.admin
            .clone()
            .oneshot(forbidden_request)
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let (_, user_cookie, user_csrf) = support::register(&f).await;
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            path,
            deletion.clone(),
            Some(&user_cookie),
            Some(&user_csrf)
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        support::request(&f.admin, 9797, "DELETE", path, deletion.clone(), None, None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            path,
            deletion.clone(),
            Some(&cookie),
            None
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            path,
            json!({"expected_revision":99}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        direct(
            &router,
            &app,
            "registered-user",
            Some("watch-a"),
            "GET",
            "/v1/external/me",
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            path,
            deletion.clone(),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            path,
            deletion,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        direct(
            &router,
            &app,
            "registered-user",
            Some("watch-a"),
            "GET",
            "/v1/external/me",
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "POST",
            &format!("{path}/key"),
            json!({"confirm":true}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let listed = support::request(
        &f.admin,
        9797,
        "GET",
        "/v1/management/external-applications",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert!(listed.body["applications"].as_array().unwrap().is_empty());
    let history = support::request(
        &f.admin,
        9797,
        "GET",
        &format!("{path}/sessions/{sid}"),
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(history.status, StatusCode::OK);
    assert_eq!(history.body["turns"][0]["prompt"], "A preserved request");
    assert_eq!(history.body["turns"][0]["answer"], "OK");
    assert_eq!(history.body["external"]["device_id"], "watch-a");
    let store = f.store.clone();
    let user = owner.clone();
    assert!(
        tokio::task::spawn_blocking(move || store.user_account(&user).unwrap().is_some())
            .await
            .unwrap()
    );
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "POST",
            "/v1/management/external-applications",
            application("private-app", "private"),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    tasks.close().await;
}

#[tokio::test]
async fn editing_application_changes_name_and_private_assistant_without_rotating_key_or_other_app()
{
    let f = support::fixture().await;
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let (router, tasks, _stop) = external(&f, &upstream).await;
    let a = create_direct_app(&f, "a", true).await;
    let b = create_direct_app(&f, "b", false).await;
    let (cookie, csrf) = support::admin_login(&f).await;
    let mut config = a["application"].clone();
    config.as_object_mut().unwrap().remove("revision");
    config["name"] = json!("Updated application");
    config["assistant_id"] = json!("private");
    config["access_ttl_seconds"] = json!(600);
    let body = json!({"application":config,"expected_revision":1,"rotate_secret":false});
    let reply = support::request(
        &f.admin,
        9797,
        "PUT",
        "/v1/management/external-applications/a",
        body.clone(),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body["application"]["name"], "Updated application");
    assert_eq!(reply.body["application"]["assistant_id"], "private");
    assert_eq!(reply.body["application"]["access_ttl_seconds"], 600);
    assert_eq!(reply.body["application"]["require_device_id"], true);
    assert_eq!(reply.body["client_secret"], "");
    assert_eq!(reply.body["application"]["revision"], 2);
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "PUT",
            "/v1/management/external-applications/a",
            body,
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::CONFLICT
    );
    let reveal = support::request(
        &f.admin,
        9797,
        "POST",
        "/v1/management/external-applications/a/key",
        json!({"confirm":true}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reveal.body["client_secret"], a["client_secret"]);
    let (status, me) = direct(
        &router,
        &a,
        "user",
        Some("watch"),
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["assistant_id"], "private");
    let (status, me) = direct(
        &router,
        &b,
        "user",
        None,
        "GET",
        "/v1/external/me",
        Value::Null,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["assistant_id"], "coder");
    let mut disabled = reply.body["application"].clone();
    disabled.as_object_mut().unwrap().remove("revision");
    disabled["enabled"] = json!(false);
    disabled["assistant_id"] = json!("inactive");
    let reply = support::request(
        &f.admin,
        9797,
        "PUT",
        "/v1/management/external-applications/a",
        json!({"application":disabled,"expected_revision":2}),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        support::request(
            &f.admin,
            9797,
            "DELETE",
            "/v1/management/external-applications/a",
            json!({"expected_revision":3}),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        direct(
            &router,
            &b,
            "user",
            None,
            "GET",
            "/v1/external/me",
            Value::Null,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(upstream.count(), 0);
    tasks.close().await;
}
