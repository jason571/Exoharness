use super::*;

fn render() -> Value {
    tool(
        "sandbox_run",
        json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"from pathlib import Path\nPath('final.txt').write_text('完整成品',encoding='utf-8')\nPath('count.txt').write_text('1')\n"},"output_files":["wrong.txt"]}),
    )
}
fn load() -> Value {
    tool(
        "activate_tools",
        json!({"names":["sandbox_run","workspace_publish"]}),
    )
}
fn publish(path: &str) -> Value {
    json!({"fixture_workspace_next":"workspace_publish","fixture_project_arguments":{"output_files":[path]}})
}
async fn close(f: Fixture) {
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn completed_code_with_wrong_output_name_recovers_only_publication() {
    let f = fixture(vec![load(), render(), publish("final.txt"), answer()], true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let execution = invocation(&f, &session, &run, 2).await;
    assert_eq!(
        execution.state,
        exoharness_contracts::invocation::InvocationState::Succeeded
    );
    let original = execution.result.unwrap();
    assert_eq!(original["execution"]["exit_code"], 0);
    assert_eq!(original["_host_publication_pending"], true);
    assert!(original["artifacts"].as_array().unwrap().is_empty());
    let delivered = invocation(&f, &session, &run, 3).await.result.unwrap();
    assert_eq!(delivered["artifacts"][0]["filename"], "final.txt");
    assert_eq!(delivered["revision"], original["revision"]);
    {
        let requests = f.upstream.requests.lock().unwrap();
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"workspace_publish") && names.contains(&"workspace_read"));
        assert!(!names.contains(&"sandbox_run") && !names.contains(&"activate_tools"));
        assert!(requests[2].to_string().contains("final.txt"));
        assert_eq!(requests.len(), 4);
    }
    let work = f
        .sandbox
        .task_store()
        .unwrap()
        .list("user", &session)
        .unwrap();
    assert_eq!(work.len(), 1);
    let count = f
        .sandbox
        .task_store()
        .unwrap()
        .read(
            "user",
            &session,
            &work[0].workspace_id,
            "count.txt",
            0,
            80,
            None,
        )
        .unwrap();
    assert_eq!(count["content"], "1");
    close(f).await;
}

#[tokio::test]
async fn pending_publication_cannot_be_reported_as_finished_text() {
    let f = fixture(vec![load(), render(), answer()], true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(
        invocation(&f, &session, &run, 2).await.result.unwrap()["execution"]["exit_code"],
        0
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
    close(f).await;
}

#[tokio::test]
async fn missing_output_reselection_is_bounded_and_never_rerenders() {
    let f = fixture(
        vec![
            load(),
            render(),
            publish("missing-1.txt"),
            publish("missing-2.txt"),
            publish("missing-3.txt"),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    for n in 3..=5 {
        let record = invocation(&f, &session, &run, n).await;
        assert_eq!(
            record.state,
            exoharness_contracts::invocation::InvocationState::RejectedBeforeExecution
        );
        assert_eq!(record.result.unwrap()["code_executed"], false);
    }
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 5);
    close(f).await;
}

#[tokio::test]
async fn publication_checkpoint_stops_a_preproposed_second_renderer() {
    let mut batch = render();
    let second = render();
    let mut again = second["choices"][0]["message"]["tool_calls"][0].clone();
    again["id"] = json!("second-render");
    batch["choices"][0]["message"]["tool_calls"]
        .as_array_mut()
        .unwrap()
        .push(again);
    let f = fixture(vec![load(), batch], true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(
        invocation(&f, &session, &run, 2).await.result.unwrap()["execution"]["exit_code"],
        0
    );
    let store = f.store.clone();
    let trace_run = run.clone();
    let trace = tokio::task::spawn_blocking(move || store.trace_run(&trace_run, 0, 1))
        .await
        .unwrap()
        .unwrap();
    let count = trace["invocations"].as_array().unwrap().len();
    assert_eq!(count, 2);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    close(f).await;
}
