use super::*;

#[tokio::test]
async fn word_skill_cjs_runtime_and_workspace_mode_are_corrected_before_one_docx_execution() {
    let args = json!({"runtime":"javascript","entrypoint":"skill/scripts/create_document.cjs","arguments":["document.json","intro.docx"],"output_files":["intro.docx"]});
    let mut wrong_runtime = args.clone();
    wrong_runtime["runtime"] = json!("python");
    let mut mixed_mode = args.clone();
    mixed_mode["skill_id"] = json!(SKILL);
    let f=fixture(vec![
        tool("read_skill",json!({"id":SKILL})),
        tool("activate_tools",json!({"names":["workspace_create","workspace_edit","sandbox_run"]})),
        tool("workspace_create",json!({"skill_id":SKILL})),
        json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":{"document.json":"{\"title\":\"ExoHarness: Rust Agent framework\"}"}}),
        json!({"fixture_workspace_next":"sandbox_run","fixture_project_arguments":wrong_runtime}),
        json!({"fixture_workspace_next":"sandbox_run","fixture_project_arguments":mixed_mode}),
        json!({"fixture_workspace_next":"sandbox_run","fixture_project_arguments":args}),answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 5).await;
    assert_eq!(
        rejected.state,
        exoharness_contracts::invocation::InvocationState::RejectedBeforeExecution
    );
    assert!(
        rejected.result.unwrap()["diagnostics"]["message"]
            .as_str()
            .unwrap()
            .contains("runtime=javascript")
    );
    let executed = invocation(&f, &session, &run, 7).await.result.unwrap();
    assert_eq!(executed["execution"]["exit_code"], 0);
    assert_eq!(executed["skill_id"], SKILL);
    assert_eq!(executed["artifacts"][0]["filename"], "intro.docx");
    let url = url::Url::parse(executed["artifacts"][0]["download_url"].as_str().unwrap()).unwrap();
    let response = exoharness_api::media_router(f.tools.artifacts.clone(), 9790)
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                .header("host", "127.0.0.1:9790")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK") && bytes.len() > 1000);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 8);
        assert!(
            requests[1]
                .to_string()
                .contains("native_workspace_contract")
        );
        assert!(requests[6].to_string().contains("omit skill_id and files"));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn missing_saved_entrypoint_is_staged_locally_before_exactly_one_execution() {
    let source = "from pathlib import Path\nPath('/workspace/report.pptx').write_bytes(b'PK'+b'fixture'*200)\n";
    let f=fixture(vec![tool("activate_tools",json!({"names":["workspace_create","workspace_edit","sandbox_run"]})),tool("workspace_create",json!({})),json!({"fixture_workspace_next":"saved_sandbox_run"}),json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":{"build_deck.py":source}}),json!({"fixture_workspace_next":"saved_sandbox_run"}),answer()],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 3).await;
    assert_eq!(
        rejected.state,
        exoharness_contracts::invocation::InvocationState::RejectedBeforeExecution
    );
    let result = rejected.result.unwrap();
    assert_eq!(result["_host_input_rejection"], 1);
    assert_eq!(result["code_executed"], false);
    assert!(
        result["diagnostics"]["message"]
            .as_str()
            .unwrap()
            .contains("build_deck.py")
    );
    let edit = invocation(&f, &session, &run, 4).await;
    assert_eq!(edit.intent.arguments["files"]["build_deck.py"], source);
    let executed = invocation(&f, &session, &run, 5).await.result.unwrap();
    assert_eq!(executed["execution"]["exit_code"], 0);
    assert_eq!(executed["artifacts"][0]["filename"], "report.pptx");
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 6);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn mixed_ppt_source_is_rejected_then_staged_exactly_and_executed_once_in_real_oci() {
    let source = "from pptx import Presentation\np=Presentation()\ns=p.slides.add_slide(p.slide_layouts[0])\ns.shapes.title.text='永嘉县特色农产品'\np.save('/workspace/report.pptx')\n";
    let files = json!({"build_deck.py":source});
    let f = fixture(
        vec![
            tool(
                "activate_tools",
                json!({"names":["workspace_create","workspace_edit","sandbox_run"]}),
            ),
            tool("workspace_create", json!({})),
            json!({"fixture_workspace_next":"mixed_sandbox_run","fixture_workspace_files":files}),
            json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":files}),
            json!({"fixture_workspace_next":"saved_sandbox_run"}),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let executed = invocation(&f, &session, &run, 5).await.result.unwrap();
    assert_eq!(executed["execution"]["exit_code"], 0);
    assert_eq!(executed["isError"], false);
    assert_eq!(executed["artifacts"][0]["filename"], "report.pptx");
    let staged = invocation(&f, &session, &run, 4).await;
    assert_eq!(staged.intent.arguments["files"]["build_deck.py"], source);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 6);
        let corrected = requests[3].to_string();
        assert!(corrected.contains("maxProperties"));
        assert!(corrected.contains("code_executed"));
        assert!(corrected.contains("build_deck.py"));
        let run_args: Value = serde_json::from_str(
            requests[4]["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert!(
            run_args.get("result").is_some() || run_args.get("completed_tool_checkpoint").is_some()
        );
    }
    let url = url::Url::parse(executed["artifacts"][0]["download_url"].as_str().unwrap()).unwrap();
    let delivered = exoharness_api::media_router(f.tools.artifacts.clone(), 9790)
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                .header("host", "127.0.0.1:9790")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delivered.status(), StatusCode::OK);
    let bytes = delivered.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK") && bytes.len() > 1000);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn unsafe_or_mismatched_entrypoints_are_corrected_without_launching_rejected_code() {
    for entrypoint in ["/skill/scripts/check.py", "../main.py", "main.sh"] {
        let source = "from pathlib import Path\nPath('final.txt').write_text('delivered')\n";
        let f = fixture(vec![
            tool("activate_tools", json!({"names":["sandbox_run"]})),
            tool("sandbox_run", json!({"runtime":"python","entrypoint":entrypoint,"files":{"main.py":source},"output_files":["final.txt"]})),
            tool("sandbox_run", json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":source},"output_files":["final.txt"]})),
            answer(),
        ], true).await;
        let (session, run) = start(&f).await;
        assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
        let rejected = invocation(&f, &session, &run, 2).await;
        assert_eq!(
            rejected.state,
            exoharness_contracts::invocation::InvocationState::RejectedBeforeExecution
        );
        let result = rejected.result.unwrap();
        assert_eq!(result["_host_input_rejection"], 1);
        assert_eq!(result["code_executed"], false);
        let executed = invocation(&f, &session, &run, 3).await.result.unwrap();
        assert_eq!(executed["execution"]["exit_code"], 0);
        assert_eq!(executed["artifacts"][0]["filename"], "final.txt");
        assert_eq!(f.upstream.requests.lock().unwrap().len(), 4);
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}
