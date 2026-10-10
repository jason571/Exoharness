#[path = "unit/model_speech.rs"]
mod model_speech;
#[path = "support/speech.rs"]
mod speech;
use exoharness_configuration::Models;
use exoharness_model_runtime::{CancellationToken, ErrorKind, ModelRuntime, RuntimePolicy};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
struct Fixture {
    _root: tempfile::TempDir,
    models: Arc<Models>,
    runtime: Arc<ModelRuntime>,
    server: speech::Server,
}
impl Fixture {
    async fn new(kind: &str, reply: speech::Reply, policy: RuntimePolicy) -> Self {
        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let server = speech::Server::new(reply).await;
        let models = Arc::new(Models::initialize(root.path()).unwrap());
        let (protocol, url) = if kind == "asr" {
            ("openai_transcription", format!("{}/v1", server.base))
        } else {
            ("volcengine_tts_sse", format!("{}/synthesize", server.base))
        };
        let mut model = json!({"name":"speech","model_id":"configured-resource","display_name":"Speech fixture","provider":"speech-api","model_type":kind,"max_tokens":1,"enabled":true});
        if kind == "tts" {
            model["tts"] = json!({"profile_id":"configured-profile","speaker":"configured-voice","sample_rate":16000,"audio_format":"mp3"});
        }
        models.save_configuration(json!({"name":"speech-api","api":protocol,"base_url":url,"model_type":kind,"enabled":true}),model,Some("fixture-key"),None).unwrap();
        let runtime = Arc::new(ModelRuntime::new(models.clone(), policy).unwrap());
        Self {
            _root: root,
            models,
            runtime,
            server,
        }
    }
}
const COMPLETE: &str = "event: chunk\r\ndata: {\r\ndata: \"code\":0,\"data\":\"SUQz\",\"message\":\"音频\"}\r\n\r\ndata: {\"code\":20000000}\r\n\r\n";
#[tokio::test]
async fn recording_timeout_cancellation_close_and_missing_model_fail_closed() {
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"data\":[{\"id\":\"another-model\"}]}"),
        RuntimePolicy::default(),
    )
    .await;
    assert_eq!(
        f.runtime
            .transcription_available("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidResponse
    );
    let mut reply = speech::Reply::json("{\"text\":\"late\"}");
    reply.delay = Duration::from_secs(5);
    *f.server.state.reply.lock().unwrap() = reply.clone();
    let runtime = f.runtime.clone();
    let cancel = CancellationToken::new();
    let task = tokio::spawn({
        let cancel = cancel.clone();
        async move {
            runtime
                .transcribe("speech", vec![1], "audio/wav", cancel)
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while f.server.count() < 2 {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    assert_eq!(task.await.unwrap().unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(f.server.count(), 2);
    assert_eq!(f.runtime.capacity().active, 0);
    f.runtime.close().await;
    assert_eq!(
        f.runtime
            .transcribe("speech", vec![1], "audio/wav", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    let f = Fixture::new(
        "asr",
        reply,
        RuntimePolicy {
            timeout: Duration::from_millis(150),
            ..RuntimePolicy::default()
        },
    )
    .await;
    assert_eq!(
        f.runtime
            .transcribe("speech", vec![1], "audio/wav", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Timeout
    );
    assert!(f.server.count() <= 1);
}
#[tokio::test]
async fn user_recording_preserves_bytes_and_mime_and_probe_checks_configured_model() {
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"data\":[{\"id\":\"configured-resource\"}]}"),
        RuntimePolicy::default(),
    )
    .await;
    f.runtime
        .transcription_available("speech", CancellationToken::new())
        .await
        .unwrap();
    *f.server.state.reply.lock().unwrap() = speech::Reply::json("{\"text\":\"用户语音\"}");
    let audio = b"browser-webm-test-exact".to_vec();
    assert_eq!(
        f.runtime
            .transcribe(
                "speech",
                audio.clone(),
                "audio/webm;codecs=opus",
                CancellationToken::new()
            )
            .await
            .unwrap(),
        "用户语音"
    );
    assert_eq!(f.server.count(), 2);
    let received = f.server.state.received.lock().unwrap();
    let (headers, body) = &received[1];
    assert_eq!(headers["authorization"], "Bearer fixture-key");
    assert!(body.windows(audio.len()).any(|part| part == audio));
    assert!(String::from_utf8_lossy(body).contains("recording.webm"));
    assert!(String::from_utf8_lossy(body).contains("audio/webm"));
}
#[tokio::test]
async fn invalid_cancelled_disabled_recordings_do_not_submit_or_retry() {
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"text\":42}"),
        RuntimePolicy::default(),
    )
    .await;
    for (audio, mime) in [
        (vec![], "audio/wav"),
        (vec![1], "text/html"),
        (
            vec![0; exoharness_model_runtime::MAX_AUDIO_BYTES + 1],
            "audio/wav",
        ),
    ] {
        assert_eq!(
            f.runtime
                .transcribe("speech", audio, mime, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::InvalidRequest
        );
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        f.runtime
            .transcribe("speech", vec![1], "audio/ogg", cancel)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    f.models.set_model_enabled("speech", false, None).unwrap();
    assert_eq!(
        f.runtime
            .transcribe("speech", vec![1], "audio/mp4", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    assert_eq!(f.server.count(), 0);
    f.models.set_model_enabled("speech", true, None).unwrap();
    assert_eq!(
        f.runtime
            .transcribe("speech", vec![1], "audio/wav", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidResponse
    );
    assert_eq!(f.server.count(), 1);
}
#[tokio::test]
async fn speech_request_budget_is_enforced_before_network_without_mutating_source() {
    let policy = RuntimePolicy {
        request_bytes: 1024,
        ..RuntimePolicy::default()
    };
    let asr = Fixture::new(
        "asr",
        speech::Reply::json("{\"text\":\"\"}"),
        policy.clone(),
    )
    .await;
    assert_eq!(
        asr.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidRequest
    );
    assert_eq!(asr.server.count(), 0);
    let tts = Fixture::new("tts", speech::Reply::sse(COMPLETE), policy).await;
    let mut model = tts.models.catalog()["models"][0].clone();
    model["tts"]["speaker"] = json!("v".repeat(512));
    model["tts"]["profile_id"] = json!("p".repeat(512));
    tts.models.save_model(model, None).unwrap();
    let original = tts.models.catalog();
    assert_eq!(
        tts.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidRequest
    );
    assert_eq!(tts.server.count(), 0);
    assert_eq!(tts.models.catalog(), original);
}
#[tokio::test]
async fn asr_uses_actual_multipart_wav_not_a_chat_request_or_health_only() {
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"text\":\"\"}"),
        RuntimePolicy::default(),
    )
    .await;
    let result = f
        .runtime
        .test("speech", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.scope, "transcription_protocol");
    assert_eq!(f.server.count(), 1);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["model"]["model_type"], "asr");
    assert_eq!(value["usage"], Value::Null);
    let received = f.server.state.received.lock().unwrap();
    let (headers, body) = &received[0];
    assert_eq!(headers["authorization"], "Bearer fixture-key");
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    assert!(body.windows(4).any(|part| part == b"RIFF"));
    assert!(body.windows(8).any(|part| part == b"WAVEfmt "));
    assert!(String::from_utf8_lossy(body).contains("configured-resource"));
    assert!(body.len() > 32044);
    assert!(!value.to_string().contains("fixture-key"));
}
#[tokio::test]
async fn tts_with_legacy_max_tokens_one_uses_declared_sse_and_requires_audio_and_completion() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(COMPLETE),
        RuntimePolicy::default(),
    )
    .await;
    let result = f
        .runtime
        .test("speech", CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.audio_bytes, Some(3));
    assert_eq!(result.scope, "speech_synthesis");
    assert_eq!(f.server.count(), 1);
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["model"]["model_type"], "tts");
    let received = f.server.state.received.lock().unwrap();
    let (headers, bytes) = &received[0];
    assert_eq!(headers["x-api-key"], "fixture-key");
    assert_eq!(headers["x-api-resource-id"], "configured-resource");
    assert!(headers.get("authorization").is_none());
    let body: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(body["req_params"]["speaker"], "configured-voice");
    assert_eq!(body["req_params"]["audio_params"]["sample_rate"], 16000);
    assert!(body.get("max_tokens").is_none());
    assert!(body.get("messages").is_none());
    assert!(!value.to_string().contains("SUQz"));
}
#[tokio::test]
async fn speech_error_empty_wrong_type_missing_terminal_and_limit_are_not_success_or_retry() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(COMPLETE),
        RuntimePolicy::default(),
    )
    .await;
    for (text, kind) in [
        (
            "data: {\"code\":0,\"data\":\"SUQz\"}\n\n",
            ErrorKind::Incomplete,
        ),
        (
            "data: {\"code\":0,\"data\":\"SUQz\"}\n\ndata: {\"code\":20000000}\n",
            ErrorKind::Incomplete,
        ),
        ("data: {\"code\":20000000}\n\n", ErrorKind::Empty),
        (
            "data: {\"code\":0,\"data\":\"!bad\"}\n\n",
            ErrorKind::InvalidResponse,
        ),
        (
            "data: {\"code\":45000001,\"message\":\"secret fixture-key\"}\n\n",
            ErrorKind::ProviderRejected,
        ),
        (
            "data: {\"code\":\"20000000\"}\n\n",
            ErrorKind::InvalidResponse,
        ),
    ] {
        *f.server.state.reply.lock().unwrap() = speech::Reply::sse(text);
        let error = f
            .runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(!error.to_string().contains("fixture-key"));
    }
    assert_eq!(f.server.count(), 6);
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"text\":42}"),
        RuntimePolicy::default(),
    )
    .await;
    assert_eq!(
        f.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidResponse
    );
    let policy = RuntimePolicy {
        response_bytes: 1024,
        ..RuntimePolicy::default()
    };
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(&format!(
            "data: {{\"code\":0,\"data\":\"{}\"}}\n\n",
            "a".repeat(2048)
        )),
        policy,
    )
    .await;
    assert_eq!(
        f.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::OutputLimit
    );
    assert_eq!(f.server.count(), 1);
}
#[tokio::test]
async fn diagnostics_respect_enable_close_timeout_and_cancel_without_second_request() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(COMPLETE),
        RuntimePolicy::default(),
    )
    .await;
    f.models.set_model_enabled("speech", false, None).unwrap();
    assert_eq!(
        f.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    assert_eq!(f.server.count(), 0);
    f.models.set_model_enabled("speech", true, None).unwrap();
    f.runtime.close().await;
    assert_eq!(
        f.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert_eq!(f.server.count(), 0);
    let mut reply = speech::Reply::json("{\"text\":\"\"}");
    reply.delay = Duration::from_secs(5);
    let f = Fixture::new("asr", reply, RuntimePolicy::default()).await;
    let cancel = CancellationToken::new();
    let runtime = f.runtime.clone();
    let task = tokio::spawn({
        let cancel = cancel.clone();
        async move { runtime.test("speech", cancel).await }
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        while f.server.count() == 0 {
            tokio::task::yield_now().await
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    assert_eq!(task.await.unwrap().unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(f.server.count(), 1);
    assert_eq!(f.runtime.capacity().active, 0);
    let policy = RuntimePolicy {
        timeout: Duration::from_millis(150),
        ..RuntimePolicy::default()
    };
    let mut reply = speech::Reply::sse(COMPLETE);
    reply.delay = Duration::from_secs(2);
    let f = Fixture::new("tts", reply, policy).await;
    assert_eq!(
        f.runtime
            .test("speech", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Timeout
    );
    assert!(f.server.count() <= 1);
}
