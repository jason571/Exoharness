use super::*;

#[tokio::test]
async fn uploaded_audio_exceeds_old_chat_limit_and_only_signed_reference_reaches_model() {
    let f = fixture(vec![answer(), answer()], true).await;
    let session = request(&f, "/v1/sessions", json!({"title":null})).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut bytes = vec![0x61; 8 * 1024 * 1024 + 512 * 1024];
    bytes[..3].copy_from_slice(b"ID3");
    let run = request(&f, &format!("/v1/sessions/{session}/turns"), json!({"prompt":"Use the uploaded audio","attachments":[{"type":"audio","name":"input.mp3","mime_type":"audio/mpeg","data":STANDARD.encode(&bytes)}]})).await["run_id"].as_str().unwrap().to_owned();
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let store = f.store.clone();
    let sid = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &sid, None, 8).unwrap()
    })
    .await
    .unwrap();
    let attachment = &detail["turns"][0]["attachments"][0];
    assert_eq!(attachment["size"], bytes.len());
    assert!(attachment.get("data").is_none());
    assert!(attachment["url"].as_str().unwrap().contains("signature="));
    let artifacts = f.tools.artifacts.clone();
    let id = attachment["attachment_id"].as_str().unwrap().to_owned();
    let copied = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let (_, mut file) = artifacts.open_owned("user", &id).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        assert!(artifacts.open_owned("other-user", &id).is_err());
        data
    })
    .await
    .unwrap();
    assert_eq!(copied, bytes);
    let next = request(
        &f,
        &format!("/v1/sessions/{session}/turns"),
        json!({"prompt":"Use the same audio again","attachments":[]}),
    )
    .await["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(terminal(&f, &next).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            let raw = request.to_string();
            assert!(raw.len() < 32 * 1024);
            assert!(raw.contains(attachment["attachment_id"].as_str().unwrap()));
            assert!(!raw.contains(&STANDARD.encode(&bytes[..512])));
        }
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn malformed_and_oversized_audio_are_rejected_before_run_or_model() {
    let f = fixture(vec![], true).await;
    let session = request(&f, "/v1/sessions", json!({"title":null})).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (data, expected) in [
        ("bad-encoding!".into(), StatusCode::BAD_REQUEST),
        (
            STANDARD.encode(vec![0u8; 32 * 1024 * 1024 + 1]),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let response = f.router.clone().oneshot(Request::builder().method("POST").uri(format!("/v1/sessions/{session}/turns")).header("host","127.0.0.1:9798").header("authorization",format!("Bearer {TOKEN}")).header("content-type","application/json").body(Body::from(json!({"prompt":"Use audio","attachments":[{"type":"audio","name":"input.mp3","data":data}]}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), expected);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(body["error"]["message"].is_string());
    }
    let store = f.store.clone();
    assert!(
        tokio::task::spawn_blocking(move || store.session_runs("user", &session, 8).unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(f.upstream.requests.lock().unwrap().is_empty());
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
