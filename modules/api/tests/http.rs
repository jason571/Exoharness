use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use exoharness_api::{ApiState, ExecutionService, Surface, auth::Credentials, router};
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;
use tower::ServiceExt;

const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ONE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TWO: &str = "cccccccccccccccccccccccccccccccc";

#[tokio::test]
async fn application_info_is_authenticated_real_build_metadata_not_config_disclosure() {
    let (admin, portal, _) = fixture().await;
    for (app, port, key) in [(&admin, 9797, ADMIN), (&portal, 9798, ONE)] {
        assert_eq!(
            call(app, port, None, "GET", "/v1/application-info", None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, value) = call(app, port, Some(key), "GET", "/v1/application-info", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            value,
            json!({"name":"ExoHarness","version":env!("CARGO_PKG_VERSION"),"product_ready":false})
        );
    }
}

#[tokio::test]
async fn preserved_console_login_cookie_csrf_and_logout_work_without_page_edits() {
    let (management, portal, _) = fixture().await;
    let login = Request::builder()
        .method("POST")
        .uri("/v1/auth/login")
        .header(header::HOST, "127.0.0.1:9797")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"password":ADMIN}).to_string()))
        .unwrap();
    let response = management.clone().oneshot(login).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let cookie = cookie.split(';').next().unwrap();
    let get = Request::builder()
        .uri("/v1/auth/session")
        .header(header::HOST, "127.0.0.1:9797")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let response = management.clone().oneshot(get).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let session: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(session["role"], "admin");
    for path in ["/overview", "/services", "/agents", "/trace"] {
        let request = Request::builder()
            .uri(path)
            .header(header::HOST, "127.0.0.1:9797")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            management.clone().oneshot(request).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let cross = Request::builder()
        .uri("/v1/auth/session")
        .header(header::HOST, "127.0.0.1:9798")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        portal.clone().oneshot(cross).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let logout = Request::builder()
        .method("POST")
        .uri("/v1/auth/logout")
        .header(header::HOST, "127.0.0.1:9797")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        management.clone().oneshot(logout).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let logout = Request::builder()
        .method("POST")
        .uri("/v1/auth/logout")
        .header(header::HOST, "127.0.0.1:9797")
        .header(header::COOKIE, cookie)
        .header("x-exoharness-csrf", session["csrf_token"].as_str().unwrap())
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        management.clone().oneshot(logout).await.unwrap().status(),
        StatusCode::OK
    );
    let expired = Request::builder()
        .uri("/v1/auth/session")
        .header(header::HOST, "127.0.0.1:9797")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        management.clone().oneshot(expired).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let request = Request::builder()
        .uri("/exoharness-login")
        .body(Body::empty())
        .unwrap();
    let response = management.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        include_bytes!("../../../web/login.html")
    );
    let request = Request::builder()
        .uri("/login")
        .body(Body::empty())
        .unwrap();
    let response = portal.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        include_bytes!("../../../web/portal-login.html")
    );
    let (status, _) = call(
        &management,
        9797,
        None,
        "POST",
        "/v1/auth/login",
        Some(json!({"password":ONE})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
async fn fixture() -> (Router, Router, watch::Sender<bool>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("disposable PostgreSQL required");
    let ns = format!(
        "pro_http_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = tokio::task::spawn_blocking(move || {
        let store = PostgresStore::connect(&url, &ns).unwrap();
        store.migrate().unwrap();
        store
    })
    .await
    .unwrap();
    let credentials=Arc::new(Credentials::from_json(&json!([{"id":"admin","role":"admin","token":ADMIN},{"id":"one","role":"user","token":ONE},{"id":"two","role":"user","token":TWO}]).to_string()).unwrap());
    let execution = Arc::new(ExecutionService(store));
    let (sender, receiver) = watch::channel(false);
    let management = router(ApiState {
        execution: execution.clone(),
        credentials: credentials.clone(),
        surface: Surface::Management,
        port: 9797,
        stop: receiver.clone(),
    });
    let portal = router(ApiState {
        execution,
        credentials,
        surface: Surface::Portal,
        port: 9798,
        stop: receiver,
    });
    (management, portal, sender)
}
async fn call(
    app: &Router,
    port: u16,
    key: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, format!("127.0.0.1:{port}"));
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let request = request
        .body(Body::from(
            body.map(|value| value.to_string()).unwrap_or_default(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, value)
}
async fn create(app: &Router) -> (String, Value) {
    let (status, data) = call(app, 9798, Some(ONE), "POST", "/v1/local/sessions", None).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = data["session_id"].as_str().unwrap().to_string();
    let (status, record) = call(
        app,
        9798,
        Some(ONE),
        "POST",
        &format!("/v1/local/sessions/{id}/runs"),
        Some(
            json!({"request_id":"request","expected_session_version":0,"input":"state validation"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(record["execution_submitted"], false);
    (id, record["run"].clone())
}

#[tokio::test]
async fn authentication_origin_host_and_role_fail_closed() {
    let (admin, portal, _stop) = fixture().await;
    assert_eq!(
        call(&portal, 9798, None, "GET", "/v1/local/status", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&admin, 9797, Some(ONE), "GET", "/v1/local/status", None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&portal, 9798, Some(ONE), "GET", "/v1/local/status", None)
            .await
            .1["principal"]["id"],
        "one"
    );
    for (host, origin, status) in [
        (
            "evil.test:9798",
            "http://127.0.0.1:9798",
            StatusCode::BAD_REQUEST,
        ),
        ("127.0.0.1:9798", "http://evil.test", StatusCode::FORBIDDEN),
    ] {
        let request = Request::builder()
            .uri("/v1/local/status")
            .header(header::HOST, host)
            .header(header::ORIGIN, origin)
            .header(header::AUTHORIZATION, format!("Bearer {ONE}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            portal.clone().oneshot(request).await.unwrap().status(),
            status
        );
    }
}

#[tokio::test]
async fn user_records_are_isolated_and_management_is_read_authorized() {
    let (admin, portal, _stop) = fixture().await;
    let (session, run) = create(&portal).await;
    let path = format!(
        "/v1/local/sessions/{session}/runs/{}",
        run["id"].as_str().unwrap()
    );
    assert_eq!(
        call(&portal, 9798, Some(TWO), "GET", &path, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &portal,
            9798,
            Some(TWO),
            "GET",
            &format!("{path}?owner=one"),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &admin,
            9797,
            Some(ADMIN),
            "GET",
            &format!("{path}?owner=one"),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&portal, 9798, Some(TWO), "GET", "/v1/local/sessions", None)
            .await
            .1["sessions"],
        json!([])
    );
    assert_eq!(
        call(&admin, 9797, Some(ADMIN), "GET", "/v1/local/sessions", None)
            .await
            .1["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn creation_is_idempotent_and_snapshot_cannot_be_client_supplied() {
    let (_admin, portal, _stop) = fixture().await;
    let (session, run) = create(&portal).await;
    let path = format!("/v1/local/sessions/{session}/runs");
    let body =
        json!({"request_id":"request","expected_session_version":0,"input":"state validation"});
    assert_eq!(
        call(&portal, 9798, Some(ONE), "POST", &path, Some(body.clone()))
            .await
            .1["run"],
        run
    );
    let mut injected = body;
    injected["snapshot"] = json!({"allowed":true});
    assert_eq!(
        call(&portal, 9798, Some(ONE), "POST", &path, Some(injected))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &portal,
            9798,
            Some(ONE),
            "POST",
            "/v1/chat",
            Some(json!({"message":"generate"}))
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        call(&portal, 9798, Some(ONE), "GET", &path, None).await.1["runs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn pause_resume_cancel_and_event_cursor_keep_one_run() {
    let (_admin, portal, _stop) = fixture().await;
    let (session, mut run) = create(&portal).await;
    let path = format!(
        "/v1/local/sessions/{session}/runs/{}",
        run["id"].as_str().unwrap()
    );
    let id = run["id"].clone();
    for (action, state) in [
        ("start", "running"),
        ("pause", "paused"),
        ("resume", "running"),
        ("cancel", "cancelled"),
    ] {
        let (status, next) = call(
            &portal,
            9798,
            Some(ONE),
            "PATCH",
            &path,
            Some(json!({"expected_version":run["version"],"action":action})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        run = next["run"].clone();
        assert_eq!(run["id"], id);
        assert_eq!(run["state"], state);
    }
    assert_eq!(
        call(
            &portal,
            9798,
            Some(ONE),
            "PATCH",
            &path,
            Some(json!({"expected_version":run["version"],"action":"resume"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let events = call(
        &portal,
        9798,
        Some(ONE),
        "GET",
        &format!("{path}/events?after=2"),
        None,
    )
    .await
    .1;
    assert_eq!(events["events"].as_array().unwrap().len(), 4);
    assert_eq!(events["events"][0]["sequence"], 3);
}

#[tokio::test]
async fn stale_versions_native_types_and_unknown_actions_are_rejected() {
    let (_admin, portal, _stop) = fixture().await;
    let (session, run) = create(&portal).await;
    let path = format!(
        "/v1/local/sessions/{session}/runs/{}",
        run["id"].as_str().unwrap()
    );
    assert_eq!(
        call(
            &portal,
            9798,
            Some(ONE),
            "PATCH",
            &path,
            Some(json!({"expected_version":0,"action":"start"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for action in [
        json!({"expected_version":"2","action":"start"}),
        json!({"expected_version":2,"action":"succeed"}),
    ] {
        assert_eq!(
            call(&portal, 9798, Some(ONE), "PATCH", &path, Some(action))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(
            &portal,
            9798,
            Some(ONE),
            "GET",
            &format!("{path}/events?limit=0"),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn sse_replays_from_cursor_and_stops_with_host() {
    let (_admin, portal, stop) = fixture().await;
    let (session, run) = create(&portal).await;
    let path = format!(
        "/v1/local/sessions/{session}/runs/{}/events/stream?after=1",
        run["id"].as_str().unwrap()
    );
    let request = Request::builder()
        .uri(path)
        .header(header::HOST, "127.0.0.1:9798")
        .header(header::AUTHORIZATION, format!("Bearer {ONE}"))
        .body(Body::empty())
        .unwrap();
    let response = portal.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let mut stream = response.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(3), stream.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = std::str::from_utf8(&frame).unwrap();
    assert!(text.contains("id: 2"));
    assert!(!text.contains("id: 1"));
    stop.send(true).unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(3), stream.frame())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn missing_sandbox_fails_closed_without_task_or_host_file() {
    let (_admin, portal, _stop) = fixture().await;
    let marker = std::env::temp_dir().join(format!("exo-no-host-exec-{}", std::process::id()));
    assert!(!marker.exists());
    for kind in ["python", "shell", "skill"] {
        let (status,result)=call(&portal,9798,Some(ONE),"POST",&format!("/v1/execute/{kind}"),Some(json!({"code":format!("touch {}",marker.display()),"command":format!("touch {}",marker.display())}))).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(result["error"]["code"], "sandbox_not_ready");
        assert_eq!(result["resumable"], false);
    }
    assert!(!marker.exists());
    assert_eq!(
        call(&portal, 9798, Some(ONE), "GET", "/v1/local/sessions", None)
            .await
            .1["sessions"],
        json!([])
    );
}
