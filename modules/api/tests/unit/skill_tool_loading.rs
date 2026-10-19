use super::*;

fn render(name: &str) -> Value {
    tool(
        "sandbox_run",
        json!({"runtime":"python","entrypoint":"main.py",
        "files":{"main.py":format!("from pathlib import Path\nPath('{name}').write_text('verified output',encoding='utf-8')\n")},
        "output_files":[name]}),
    )
}

async fn close(f: Fixture) {
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn skill_read_loads_authorized_schemas_without_a_search_or_activation_round() {
    let f = fixture(
        vec![
            tool(
                "read_skill",
                json!({"id":SKILL,"load_tools":["sandbox_run"]}),
            ),
            render("final.txt"),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let read = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert!(
        read["content"]
            .as_str()
            .unwrap()
            .contains("Use sandbox_run")
    );
    assert_eq!(read["_host_execution"], false);
    assert_eq!(read["tool_selection"]["loaded"], json!(["sandbox_run"]));
    let execution = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(execution["execution"]["exit_code"], 0);
    assert_eq!(execution["artifacts"].as_array().unwrap().len(), 1);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1]["tools"].as_array().unwrap().len(), 4);
    }
    close(f).await;
}

#[tokio::test]
async fn rejected_combined_selection_keeps_the_first_instruction_read_available() {
    let f = fixture(
        vec![
            tool(
                "read_skill",
                json!({"id":SKILL,"load_tools":["sandbox_run","unknown-tool"]}),
            ),
            tool(
                "read_skill",
                json!({"id":SKILL,"load_tools":["sandbox_run"]}),
            ),
            render("final.txt"),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(rejected["_host_input_rejection"], 1);
    assert!(rejected.get("content").is_none());
    let read = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert!(
        read["content"]
            .as_str()
            .unwrap()
            .contains("Use sandbox_run")
    );
    assert_ne!(read["already_loaded"], true);
    let requests = f.upstream.requests.lock().unwrap();
    assert_eq!(requests[1]["tools"].as_array().unwrap().len(), 3);
    assert_eq!(requests[2]["tools"].as_array().unwrap().len(), 4);
    drop(requests);
    close(f).await;
}

#[tokio::test]
async fn skill_read_cannot_load_a_denied_execution_tool() {
    let f = fixture(
        vec![tool(
            "read_skill",
            json!({"id":SKILL,"load_tools":["sandbox_run"]}),
        )],
        false,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(rejected["_host_authorization_denied"], 1);
    assert_eq!(rejected["_host_execution"], false);
    assert!(rejected.get("content").is_none());
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 1);
    close(f).await;
}

#[tokio::test]
async fn combined_loading_preserves_existing_tools_and_independent_outputs_in_a_batch() {
    let mut batch = render("first.txt");
    let second = render("second.txt");
    let mut call = second["choices"][0]["message"]["tool_calls"][0].clone();
    call["id"] = json!("independent-second-output");
    batch["choices"][0]["message"]["tool_calls"]
        .as_array_mut()
        .unwrap()
        .push(call);
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":["workspace_read"]})),
            tool(
                "read_skill",
                json!({"id":SKILL,"load_tools":["sandbox_run"]}),
            ),
            batch,
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let read = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(
        read["tool_selection"]["loaded"],
        json!(["sandbox_run", "workspace_read"])
    );
    let store = f.store.clone();
    let sid = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &sid, None, 8).unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        detail["turns"][0]["response"]["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 4);
    close(f).await;
}
