use super::*;
use exoharness_contracts::model::ToolCall;
use exoharness_model_runtime::CancellationToken;

async fn speech_model(f: &Fixture, kind: &str) -> speech_fixture::Server {
    let reply = if kind == "tts" {
        speech_fixture::Reply::sse(
            "data: {\"code\":0,\"data\":\"SUQz\"}\n\ndata: {\"code\":20000000}\n\n",
        )
    } else {
        speech_fixture::Reply::json("{\"text\":\"模型实际识别文字\"}")
    };
    let server = speech_fixture::Server::new(reply).await;
    let (api, url) = if kind == "tts" {
        ("volcengine_tts_sse", format!("{}/synthesize", server.base))
    } else {
        ("openai_transcription", format!("{}/v1", server.base))
    };
    let mut model = json!({"name":kind,"model_type":kind,"provider":kind,"model_id":"test-model","display_name":"test","enabled":true,"max_tokens":1});
    if kind == "tts" {
        model["tts"] = json!({"profile_id":"test-profile","speaker":"test-speaker"});
    }
    f.models
        .save_configuration(
            json!({"name":kind,"model_type":kind,"api":api,"base_url":url,"enabled":true}),
            model,
            Some("test-private-key"),
            None,
        )
        .unwrap();
    server
}
fn profile(mode: &str) -> Value {
    json!({"access":{"models":{"mode":mode},"tools":{"mode":"none"},"skills":{"mode":"none"},"mcp":{"mode":"none"}},"active_revision":1})
}
async fn catalog(f: &Fixture, mode: &str) -> exoharness_tools::Catalog {
    let tools = f.tools.clone();
    let profile = profile(mode);
    tokio::task::spawn_blocking(move || tools.catalog("user", profile).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn enabled_model_is_discoverable_without_registry_row_and_none_is_denied() {
    let f = fixture(vec![], false).await;
    let server = speech_model(&f, "tts").await;
    let allowed = catalog(&f, "all").await;
    let found = allowed.search("speech", 0);
    assert!(
        found["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "model_synthesize_speech")
    );
    let mut denied = catalog(&f, "none").await;
    assert!(
        denied
            .activate(&["model.synthesize_speech".into()])
            .is_err()
    );
    assert_eq!(server.count(), 0);
    f.models.set_model_enabled("tts", false, None).unwrap();
    let mut disabled = catalog(&f, "all").await;
    assert!(
        disabled
            .activate(&["model.synthesize_speech".into()])
            .is_err()
    );
}

#[tokio::test]
async fn speech_is_persisted_without_base64_internal_card_suppressed_and_asr_uses_owned_file() {
    let f = fixture(vec![answer()], false).await;
    let _tts = speech_model(&f, "tts").await;
    let asr = speech_model(&f, "asr").await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let mut catalog = catalog(&f, "all").await;
    catalog
        .activate(&[
            "model.synthesize_speech".into(),
            "model.transcribe_audio".into(),
        ])
        .unwrap();
    let (tx, _) = watch::channel(None);
    let speech=f.tools.dispatch(&mut catalog,"user",&session,&run,"tts-invocation",&ToolCall{id:"tts-call".into(),name:"model_synthesize_speech".into(),arguments:json!({"text":"讲解内容","filename":"lesson.mp3","artifact_visibility":"internal"})},CancellationToken::new(),tx.clone()).await.unwrap();
    assert!(speech.executed && !speech.failed);
    assert!(speech.artifacts.is_empty());
    let id = speech.value["artifact_id"].as_str().unwrap().to_owned();
    assert!(!speech.value.to_string().contains("SUQz"));
    let arts = f.tools.artifacts.clone();
    let own_id = id.clone();
    tokio::task::spawn_blocking(move || {
        let (_, mut file) = arts.open_owned("user", &own_id).unwrap();
        use std::io::Read;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"ID3");
        assert!(arts.open_owned("other-user", &own_id).is_err());
    })
    .await
    .unwrap();
    let recognized = f
        .tools
        .dispatch(
            &mut catalog,
            "user",
            &session,
            &run,
            "asr-invocation",
            &ToolCall {
                id: "asr-call".into(),
                name: "model_transcribe_audio".into(),
                arguments: json!({"artifact_id":id}),
            },
            CancellationToken::new(),
            tx,
        )
        .await
        .unwrap();
    assert_eq!(recognized.value["text"], "模型实际识别文字");
    assert_eq!(asr.count(), 1);
}
