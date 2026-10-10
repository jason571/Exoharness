use super::*;
#[tokio::test]
async fn configured_tts_collects_audio_and_uses_exact_text_and_settings() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(COMPLETE),
        RuntimePolicy::default(),
    )
    .await;
    let descriptors = f.runtime.speech_models().unwrap();
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].name, "speech");
    let audio = f
        .runtime
        .synthesize(
            "speech",
            descriptors[0].revision,
            "一年级的讲解",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(audio.bytes, b"ID3");
    assert_eq!(audio.extension, "mp3");
    let received = f.server.state.received.lock().unwrap();
    let value: Value = serde_json::from_slice(&received[0].1).unwrap();
    assert_eq!(value["req_params"]["text"], "一年级的讲解");
    assert_eq!(value["req_params"]["speaker"], "configured-voice");
    assert_eq!(f.server.count(), 1);
}
#[tokio::test]
async fn speech_revision_change_disable_and_cancel_never_submit() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse(COMPLETE),
        RuntimePolicy::default(),
    )
    .await;
    let revision = f.runtime.speech_models().unwrap()[0].revision;
    assert_eq!(
        f.runtime
            .synthesize("speech", revision + 1, "你好", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        f.runtime
            .synthesize("speech", revision, "你好", cancel)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    f.models.set_model_enabled("speech", false, None).unwrap();
    assert!(f.runtime.speech_models().unwrap().is_empty());
    assert_eq!(
        f.runtime
            .synthesize("speech", revision, "你好", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    assert_eq!(f.server.count(), 0);
}
#[tokio::test]
async fn truncated_synthesis_is_not_success_and_has_no_hidden_retry() {
    let f = Fixture::new(
        "tts",
        speech::Reply::sse("data: {\"code\":0,\"data\":\"SUQz\"}\n\n"),
        RuntimePolicy::default(),
    )
    .await;
    let revision = f.runtime.speech_models().unwrap()[0].revision;
    assert_eq!(
        f.runtime
            .synthesize("speech", revision, "你好", CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Incomplete
    );
    assert_eq!(f.server.count(), 1);
}
#[tokio::test]
async fn configured_asr_has_a_revision_fence_and_returns_actual_transcript() {
    let f = Fixture::new(
        "asr",
        speech::Reply::json("{\"text\":\"人工智能是机器小帮手\"}"),
        RuntimePolicy::default(),
    )
    .await;
    let revision = f.runtime.speech_models().unwrap()[0].revision;
    assert_eq!(
        f.runtime
            .transcribe_revision(
                "speech",
                revision + 1,
                vec![1, 2, 3],
                "audio/wav",
                CancellationToken::new()
            )
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    assert_eq!(f.server.count(), 0);
    assert_eq!(
        f.runtime
            .transcribe_revision(
                "speech",
                revision,
                vec![1, 2, 3],
                "audio/wav",
                CancellationToken::new()
            )
            .await
            .unwrap(),
        "人工智能是机器小帮手"
    );
    assert_eq!(f.server.count(), 1);
}
