#[path = "../../model-runtime/tests/support/server.rs"]
#[allow(dead_code)]
mod protocol_server;
#[path = "../../model-runtime/tests/support/speech.rs"]
mod speech_server;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{
    ApiState, ExecutionService, Surface, auth::Credentials, configuration_router_with_runtime,
};
use exoharness_configuration::{Assistants, Models};
use exoharness_model_runtime::{ModelRuntime, RuntimePolicy};
use exoharness_storage_postgres::PostgresStore;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
#[tokio::test]
async fn browser_speech_routes_authenticate_and_accept_raw_audio_not_chat_json() {
    let llm =
        protocol_server::Server::new(protocol_server::Reply::json(protocol_server::answer())).await;
    let (_root, models, router) = fixture(&llm).await;
    let server =
        speech_server::Server::new(speech_server::Reply::json("{\"text\":\"语音测试\"}")).await;
    models.save_configuration(json!({"name":"asr-api","api":"openai_transcription","base_url":format!("{}/v1",server.base),"model_type":"asr","enabled":true}),json!({"name":"asr","provider":"asr-api","model_id":"speech-fixture","display_name":"ASR","model_type":"asr","enabled":true,"max_tokens":1}),Some("private-test-credential"),None).unwrap();
    models.set_default("asr", None).unwrap();
    for (token, bytes, mime, status) in [
        (None, vec![1], "audio/webm", StatusCode::UNAUTHORIZED),
        (Some(ADMIN), vec![], "audio/webm", StatusCode::BAD_REQUEST),
        (
            Some(ADMIN),
            vec![1],
            "application/json",
            StatusCode::BAD_REQUEST,
        ),
        (
            Some(ADMIN),
            vec![1, 2, 3],
            "audio/webm;codecs=opus",
            StatusCode::OK,
        ),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/speech-input")
            .header("host", "127.0.0.1:9797")
            .header("content-type", mime);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::from(bytes)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        if status == StatusCode::OK {
            let value: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(value, json!({"text":"语音测试"}));
        }
    }
    assert_eq!(server.count(), 1);
    assert_eq!(llm.count(), 0);
}
async fn fixture(server: &protocol_server::Server) -> (tempfile::TempDir, Arc<Models>, Router) {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let models = Arc::new(Models::initialize(root.path()).unwrap());
    models.save_configuration(
        json!({"name":"fixture-api","api":"openai_compatible","base_url":server.url,"enabled":true}),
        json!({"name":"llm","provider":"fixture-api","model_id":"fixture-model","display_name":"Mock","enabled":true,"max_tokens":128,"thinking":"none","top_k":40}),
        Some("private-test-credential"),None).unwrap();
    let store = tokio::task::spawn_blocking(|| {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let namespace = format!(
            "pro_model_adapter_{}_{}",
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
                {"id":"admin","role":"admin","token":ADMIN},
                {"id":"user","role":"user","token":USER}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let (_, stop) = tokio::sync::watch::channel(false);
    let state = ApiState {
        execution: Arc::new(ExecutionService(store)),
        credentials,
        surface: Surface::Management,
        port: 9797,
        stop,
    };
    let runtime = Arc::new(ModelRuntime::new(models.clone(), RuntimePolicy::default()).unwrap());
    let router = configuration_router_with_runtime(
        state,
        models.clone(),
        Arc::new(Assistants::empty()),
        None,
        Some(runtime),
    );
    (root, models, router)
}
async fn call(router: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    call_named(router, token, "llm", body).await
}
async fn call_named(router: &Router, token: &str, name: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/models/{name}/test"))
                .header("host", "127.0.0.1:9797")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}
#[tokio::test]
async fn tts_asr_original_api_dispatches_by_type_and_does_not_apply_llm_token_rules() {
    let llm =
        protocol_server::Server::new(protocol_server::Reply::json(protocol_server::answer())).await;
    let (_root, models, router) = fixture(&llm).await;
    for kind in ["tts", "asr"] {
        let reply = if kind == "tts" {
            speech_server::Reply::sse(
                "data: {\"code\":0,\"data\":\"SUQz\"}\n\ndata: {\"code\":20000000}\n\n",
            )
        } else {
            speech_server::Reply::json("{\"text\":\"\"}")
        };
        let speech = speech_server::Server::new(reply).await;
        let protocol = if kind == "tts" {
            "volcengine_tts_sse"
        } else {
            "openai_transcription"
        };
        let url = format!(
            "{}/{}",
            speech.base,
            if kind == "tts" { "synthesize" } else { "v1" }
        );
        let name = format!("speech-{kind}");
        let provider = format!("api-{kind}");
        let mut model = json!({"name":name,"display_name":"Speech fixture","model_id":"speech-fixture","provider":provider,"model_type":kind,"max_tokens":1,"enabled":true});
        if kind == "tts" {
            model["tts"] = json!({"profile_id":"test-profile","speaker":"test-speaker"});
        }
        models.save_configuration(json!({"name":provider,"api":protocol,"base_url":url,"model_type":kind,"enabled":true}),model,Some("speech-secret-fixture"),None).unwrap();
        assert_eq!(
            call_named(&router, USER, &name, json!({"confirm":true}))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call_named(&router, ADMIN, &name, json!({"confirm":false}))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(speech.count(), 0);
        let (status, result) = call_named(&router, ADMIN, &name, json!({"confirm":true})).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["model"]["model_type"], kind);
        assert_eq!(speech.count(), 1);
        assert_eq!(result["tested"], true);
        assert_eq!(result["agent_integrated"], false);
        if kind == "tts" {
            assert_eq!(result["audio_bytes"], 3);
        } else {
            assert_eq!(result["scope"], "transcription_protocol");
        }
        assert!(!result.to_string().contains("speech-secret-fixture"));
        assert!(result.get("content_base64").is_none());
    }
    assert_eq!(llm.count(), 0);
}
#[tokio::test]
async fn original_test_action_calls_actual_adapter_and_preserves_auth_confirmation_and_usage() {
    let server =
        protocol_server::Server::new(protocol_server::Reply::json(protocol_server::answer())).await;
    let (_root, models, router) = fixture(&server).await;
    assert_eq!(
        call(&router, USER, json!({"confirm":true})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&router, ADMIN, json!({"confirm":false})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&router, ADMIN, json!({"confirm":"true"})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(server.count(), 0);
    let (status, body) = call(
        &router,
        ADMIN,
        json!({"actor":"web-console","confirm":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server.count(), 1);
    assert_eq!(body["model"]["name"], "llm");
    assert_eq!(body["model"]["provider"], "fixture-api");
    assert_eq!(body["configuration_revision"], 1);
    assert_eq!(body["tested"], true);
    assert_eq!(body["agent_integrated"], false);
    assert_eq!(body["usage"]["input_tokens"], 12);
    assert_eq!(server.state.received.lock().unwrap()[0].1["top_k"], 40);
    assert!(!body.to_string().contains("private-test-credential"));
    models.set_model_enabled("llm", false, None).unwrap();
    assert_eq!(
        call(&router, ADMIN, json!({"confirm":true})).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(server.count(), 1);
}
#[tokio::test]
async fn rate_limit_empty_and_truncation_are_not_success_or_implicit_retry() {
    let server =
        protocol_server::Server::new(protocol_server::Reply::json(protocol_server::answer())).await;
    let (_root, _models, router) = fixture(&server).await;
    let mut reply = protocol_server::Reply::json(json!({"error":"secret provider body"}));
    reply.status = StatusCode::TOO_MANY_REQUESTS;
    reply.retry_after = Some("60");
    *server.state.reply.lock().unwrap() = reply;
    let (status, body) = call(&router, ADMIN, json!({"confirm":true})).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["resumable"], false);
    assert!(!body.to_string().contains("secret provider body"));
    let mut value = protocol_server::answer();
    value["choices"][0]["finish_reason"] = json!("length");
    *server.state.reply.lock().unwrap() = protocol_server::Reply::json(value);
    let (status, body) = call(&router, ADMIN, json!({"confirm":true})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["code"], "model_output_limit");
    let mut value = protocol_server::answer();
    value["choices"][0]["message"]["content"] = json!(" ");
    *server.state.reply.lock().unwrap() = protocol_server::Reply::json(value);
    assert_eq!(
        call(&router, ADMIN, json!({"confirm":true})).await.0,
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(server.count(), 3);
}
