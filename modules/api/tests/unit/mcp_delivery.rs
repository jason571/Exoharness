use super::*;

#[tokio::test]
async fn declared_mcp_audio_uses_permanent_media_delivery_preserves_source_and_supports_range() {
    let remote = mcp_server::server(mcp_server::StateData {
        declared_audio: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"generate once"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start_prompt(&f, "生成歌曲").await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let receipt = invocation(&f, &session, &run, 2).await.result.unwrap();
    let raw = receipt["content"][0]["uri"].as_str().unwrap();
    assert!(raw.starts_with(remote.url.trim_end_matches("/mcp")));
    let store = f.store.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store
            .conversation_detail("user", &session, None, 8)
            .unwrap()
    })
    .await
    .unwrap();
    let card = &detail["turns"][0]["response"]["artifacts"][0];
    assert_eq!(card["content_type"], "audio/mpeg");
    let url = url::Url::parse(card["url"].as_str().unwrap()).unwrap();
    assert!(url.as_str().starts_with("http://127.0.0.1:9790/artifacts/"));
    assert!(!url.as_str().contains("file-only"));
    assert_eq!(receipt["_host_delivered_artifacts"][0]["url"], card["url"]);
    let response = exoharness_api::media_router(f.tools.artifacts.clone(), 9790)
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                .header("host", "127.0.0.1:9790")
                .header("range", "bytes=0-8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-type"], "audio/mpeg");
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("inline")
    );
    assert_eq!(
        &response.into_body().collect().await.unwrap().to_bytes()[..],
        b"completed"
    );
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        1
    );
    assert_eq!(remote.state.file_headers.lock().unwrap().len(), 1);
    assert!(!remote.state.file_headers.lock().unwrap()[0].contains_key("authorization"));
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
