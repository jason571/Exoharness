use super::*;
use exoharness_contracts::model::ToolCall;
use exoharness_model_runtime::CancellationToken;
async fn dispatch(
    f: &Fixture,
    catalog: &mut exoharness_tools::Catalog,
    session: &str,
    run: &str,
    name: &str,
    args: Value,
    key: &str,
) -> exoharness_tools::Result<exoharness_tools::DispatchResult> {
    let (tx, _) = watch::channel(None);
    f.tools
        .dispatch(
            catalog,
            "user",
            session,
            run,
            key,
            &ToolCall {
                id: key.into(),
                name: name.into(),
                arguments: args,
            },
            CancellationToken::new(),
            tx,
        )
        .await
}
#[tokio::test]
async fn native_workspace_reuses_sources_patches_publishes_and_imports_scoped_artifacts() {
    let f = fixture(vec![answer()], true).await;
    let (session, run) = start(&f).await;
    terminal(&f, &run).await;
    let tools = f.tools.clone();
    let mut catalog = tokio::task::spawn_blocking(move || {
        tools.catalog(
            "user",
            json!({"access":{"tools":{"mode":"all"},"skills":{"mode":"all"},"mcp":{"mode":"all"}}}),
        )
    })
    .await
    .unwrap()
    .unwrap();
    dispatch(&f,&mut catalog,&session,&run,"activate_tools",json!({"names":["workspace_create","workspace_edit","workspace_read","sandbox_run","workspace_publish","workspace_import","workspace_list","workspace_job"]}),"load").await.unwrap();
    let created = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_create",
        json!({}),
        "task-create",
    )
    .await
    .unwrap();
    let id = created.value["workspace_id"].as_str().unwrap();
    let saved=dispatch(&f,&mut catalog,&session,&run,"workspace_edit",json!({"workspace_id":id,"expected_revision":1,"files":{"main.py":"from pathlib import Path\nPath('final.txt').write_text('中文完整输出',encoding='utf-8')\nprint('done')"}}),"task-edit").await.unwrap();
    assert_eq!(saved.value["revision"], 2);
    let executed=dispatch(&f,&mut catalog,&session,&run,"sandbox_run",json!({"workspace_id":id,"expected_revision":2,"runtime":"python","entrypoint":"main.py","output_files":[]}),"task-execute").await.unwrap();
    assert_eq!(executed.value["revision"], 3);
    assert_eq!(executed.value["execution"]["exit_code"], 0);
    assert!(executed.artifacts.is_empty());
    let read = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_read",
        json!({"workspace_id":id,"path":"final.txt"}),
        "task-read",
    )
    .await
    .unwrap();
    assert_eq!(read.value["content"], "中文完整输出");
    let published = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_publish",
        json!({"workspace_id":id,"expected_revision":3,"output_files":["final.txt"]}),
        "task-publish",
    )
    .await
    .unwrap();
    assert_eq!(published.artifacts.len(), 1);
    let artifact = published.artifacts[0]["id"].as_str().unwrap();
    let target = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_create",
        json!({}),
        "target-create",
    )
    .await
    .unwrap();
    let target_id = target.value["workspace_id"].as_str().unwrap();
    let imported=dispatch(&f,&mut catalog,&session,&run,"workspace_import",json!({"workspace_id":target_id,"expected_revision":1,"inputs":[{"path":"input.txt","artifact_id":artifact}]}),"target-import").await.unwrap();
    assert_eq!(imported.value["revision"], 2);
    let read = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_read",
        json!({"workspace_id":target_id,"path":"input.txt"}),
        "input-read",
    )
    .await
    .unwrap();
    assert_eq!(read.value["content"], "中文完整输出");
    assert!(
        dispatch(
            &f,
            &mut catalog,
            "another-session",
            &run,
            "workspace_read",
            json!({"workspace_id":id}),
            "cross-session"
        )
        .await
        .is_err()
    );
    let service = f.tools.artifacts.clone();
    let denied_artifact = artifact.to_owned();
    assert!(
        tokio::task::spawn_blocking(move || service.open_owned("another-user", &denied_artifact))
            .await
            .unwrap()
            .is_err()
    );
    assert!(dispatch(&f,&mut catalog,&session,&run,"workspace_import",json!({"workspace_id":target_id,"expected_revision":2,"inputs":[{"path":"input.txt","artifact_id":artifact}]}),"overwrite-input").await.is_err());
    let jobs = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_job",
        json!({"workspace_id":id,"invocation_id":"task-execute","action":"status"}),
        "job-status",
    )
    .await
    .unwrap();
    assert_eq!(jobs.value["execution"]["cleanup_confirmed"], true);
    let listed = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_list",
        json!({}),
        "list",
    )
    .await
    .unwrap();
    assert_eq!(listed.value["workspaces"].as_array().unwrap().len(), 2);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn successful_execution_with_wrong_output_name_is_published_without_rerunning_code() {
    let f = fixture(vec![answer()], true).await;
    let (session, run) = start(&f).await;
    terminal(&f, &run).await;
    let tools = f.tools.clone();
    let mut catalog = tokio::task::spawn_blocking(move || {
        tools.catalog(
            "user",
            json!({"access":{"tools":{"mode":"all"},"skills":{"mode":"all"},"mcp":{"mode":"all"}}}),
        )
    })
    .await
    .unwrap()
    .unwrap();
    dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "activate_tools",
        json!({"names":["sandbox_run","workspace_read","workspace_publish"]}),
        "load-publication",
    )
    .await
    .unwrap();
    let source = "from pathlib import Path\ncounter=Path('count.txt')\ncounter.write_text(str(int(counter.read_text())+1) if counter.exists() else '1')\nPath('final.txt').write_text('完整成品',encoding='utf-8')";
    let executed = dispatch(&f, &mut catalog, &session, &run, "sandbox_run", json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":source},"output_files":["wrong-name.txt"]}), "execute-once").await.unwrap();
    assert!(executed.executed);
    assert!(!executed.failed);
    assert_eq!(executed.value["_host_publication_pending"], true);
    assert_eq!(executed.value["execution"]["exit_code"], 0);
    assert!(executed.value["publication_error"].is_string());
    assert!(
        executed.value["recovery"]
            .as_str()
            .unwrap()
            .contains("workspace_publish")
    );
    assert!(executed.artifacts.is_empty());
    let id = executed.value["workspace_id"].as_str().unwrap();
    let revision = executed.value["revision"].as_u64().unwrap();
    let published = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_publish",
        json!({"workspace_id":id,"expected_revision":revision,"output_files":["final.txt"]}),
        "publish-saved-output",
    )
    .await
    .unwrap();
    assert!(!published.failed);
    assert_eq!(published.artifacts.len(), 1);
    assert_eq!(published.value["_host_publication_complete"], true);
    assert_eq!(published.value["revision"], revision);
    let count = dispatch(
        &f,
        &mut catalog,
        &session,
        &run,
        "workspace_read",
        json!({"workspace_id":id,"path":"count.txt"}),
        "verify-not-rerendered",
    )
    .await
    .unwrap();
    assert_eq!(count.value["content"], "1");
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn persisted_source_checkpoint_replaces_large_arguments_only_after_journal_commit() {
    let source = format!(
        "# {}\nprint('ok')",
        "saved source not model history ".repeat(3500)
    );
    let f=fixture(vec![tool("activate_tools",json!({"names":["sandbox_run"]})),tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":source},"output_files":[]})),answer()],true).await;
    let (_session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let requests = f.upstream.requests.lock().unwrap().clone();
    let final_request = requests.last().unwrap().to_string();
    assert!(final_request.contains("completed_tool_checkpoint"));
    assert!(final_request.contains("urn:exoharness:invocation:"));
    assert!(!final_request.contains("_persisted_arguments_reference"));
    assert!(!final_request.contains("saved source not model history"));
    let state = f
        .sandbox
        .task_store()
        .unwrap()
        .list("user", &_session)
        .unwrap();
    assert_eq!(state.len(), 1);
    assert!(state[0].files[0].size_bytes > 100000);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
