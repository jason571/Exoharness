use super::*;

#[tokio::test]
async fn mixed_full_and_incomplete_tool_names_are_corrected_together_then_execute_once() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let provider = "mcp_server:global:_:fixture";
    let first = exoharness_tools::public_name(provider, "echo");
    let second = exoharness_tools::public_name(provider, "other");
    let f = fixture(
        vec![
            tool(
                "activate_tools",
                json!({"names":[first,"mcp_other"],"mode":"add"}),
            ),
            tool(
                "activate_tools",
                json!({"names":[first,second],"mode":"add"}),
            ),
            tool(&first, json!({"text":"execute once; preserve exact input"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(rejected["_host_input_rejection"], 1);
    assert_eq!(rejected["code_executed"], false);
    let diagnostic = rejected["diagnostics"]["message"].as_str().unwrap();
    assert!(diagnostic.contains(&first) && diagnostic.contains(&second));
    assert!(diagnostic.contains("\"mode\":\"add\""));
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 3);
        assert_eq!(requests[1]["tools"].as_array().unwrap().len(), 3);
        assert_eq!(requests[2]["tools"].as_array().unwrap().len(), 5);
    }
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
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn unchanged_incomplete_selection_stops_without_repeat_dispatch_or_mcp_execution() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":["mcp_echo"]})),
            tool("activate_tools", json!({"names":["mcp_echo"]})),
            tool("activate_tools", json!({"names":["mcp_echo"]})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .iter()
                .all(|r| r["tools"].as_array().unwrap().len() == 3)
        );
    }
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        0
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn authorized_service_id_mistake_is_locally_corrected_without_widening_or_executing_tools() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let provider = "mcp_server:global:_:fixture";
    let first = exoharness_tools::public_name(provider, "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[provider]})),
            tool(
                "search_capabilities",
                json!({"query":"","kind":"tool","capability_id":provider,"names":["echo"]}),
            ),
            tool("activate_tools", json!({"names":[first]})),
            tool(&first, json!({"text":"ONLY the requested action"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(rejected["_host_input_rejection"], 1);
    assert_eq!(rejected["code_executed"], false);
    assert_eq!(rejected["_host_execution"], false);
    assert!(
        rejected["diagnostics"]["message"]
            .as_str()
            .unwrap()
            .contains("capability_id")
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        for request in requests.iter().take(3) {
            assert_eq!(request["tools"].as_array().unwrap().len(), 3);
        }
        assert_eq!(requests[3]["tools"].as_array().unwrap().len(), 4);
        assert_eq!(requests.len(), 5);
    }
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
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn service_selection_exact_lookup_and_durable_stage_load_only_current_schemas() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        extra_tools: 66,
        ..Default::default()
    })
    .await;
    let provider = "mcp_server:global:_:fixture";
    let first = exoharness_tools::public_name(provider, "echo");
    let second = exoharness_tools::public_name(provider, "other");
    let f = fixture(
        vec![
            tool(
                "search_capabilities",
                json!({"query":"","kind":"capability"}),
            ),
            tool(
                "search_capabilities",
                json!({"query":"","kind":"tool","capability_id":provider,"names":["echo"]}),
            ),
            tool("activate_tools", json!({"names":[first]})),
            tool(&first, json!({"text":"EXACT authored plan"})),
            tool(&second, json!({"state_id":"saved-1"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let index = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(index["total"], 2); // MCP and native sandbox, not 68 MCP tools.
    assert!(!index.to_string().contains("unused_65"));
    let exact = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(exact["total"], 1);
    assert_eq!(exact["items"][0]["name"], first);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 6);
        for (i, request) in requests.iter().enumerate() {
            let tools = request["tools"].as_array().unwrap();
            assert_eq!(tools.len(), if i == 3 || i == 4 { 4 } else { 3 });
            assert!(
                !request["tools"]
                    .to_string()
                    .contains("UNRELATED_FULL_SCHEMA")
            );
        }
        let before_execution = requests[3]["messages"].to_string();
        assert!(before_execution.contains("catalog_selection_retired"));
        assert!(!before_execution.contains("unused_65"));
        assert!(
            requests[4]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == second)
        );
        assert!(
            !requests[4]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == first)
        );
    }
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        2
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn stage_batch_and_explicit_add_preserve_authorization_schema_and_exact_business_inputs() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let first = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let second = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let authored = "KEEP ALL STORYBOARD AND NARRATION DETAILS ".repeat(250);
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":["echo"]})),
            tool("activate_tools", json!({"names":["other"],"mode":"add"})),
            tool(&first, json!({"text":authored})),
            tool(
                &second,
                json!({"storyboard":[{"shot":1,"prompt":"EXACT SECOND STEP"}]}),
            ),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    assert_eq!(
        invocation(&f, &session, &run, 3).await.intent.arguments["text"],
        authored
    );
    assert_eq!(
        invocation(&f, &session, &run, 4).await.intent.arguments["storyboard"][0]["prompt"],
        "EXACT SECOND STEP"
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&first.as_str()) && names.contains(&second.as_str()));
        assert!(!names.contains(&"sandbox_run"));
        assert_eq!(requests[2]["tools"].as_array().unwrap().len(), 5);
        assert_eq!(requests.len(), 5);
    }
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        2
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn distinct_pages_and_duplicate_reads_preserve_exact_journal_and_publication() {
    for (offsets, expected_bodies, expected_references) in
        [([0, 80, 160, 240], 4, 0), ([0, 80, 0, 80], 2, 2)]
    {
        let f = fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","workspace_edit","workspace_read","workspace_publish"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"workspace_edit"}),
        json!({"fixture_workspace_next":"workspace_read","fixture_workspace_offset":offsets[0]}),
        json!({"fixture_workspace_next":"workspace_read","fixture_workspace_offset":offsets[1]}),
        json!({"fixture_workspace_next":"workspace_read","fixture_workspace_offset":offsets[2]}),
        json!({"fixture_workspace_next":"workspace_read","fixture_workspace_offset":offsets[3]}),
        json!({"fixture_workspace_next":"workspace_publish"}),
        answer(),
    ],true).await;
        let (session, run) = start(&f).await;
        assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
        {
            let requests = f.upstream.requests.lock().unwrap();
            let mut read_bodies = 0;
            let mut read_references = 0;
            for message in requests[7]["messages"].as_array().unwrap() {
                if message["role"] != "tool" {
                    continue;
                }
                let Ok(receipt) =
                    serde_json::from_str::<Value>(message["content"].as_str().unwrap())
                else {
                    continue;
                };
                let result = &receipt["result"];
                if result["_durable_read_body"] == true {
                    read_bodies += usize::from(result["content"].is_string());
                    read_references += usize::from(result["read_body_retained_in_journal"] == true);
                    assert_eq!(result["revision"], 2);
                    assert!(result["file"]["sha256"].is_string());
                }
            }
            assert_eq!(read_bodies, expected_bodies);
            assert_eq!(read_references, expected_references);
            assert!(
                !requests[7]["messages"].to_string().contains(
                    "\"role\":\"assistant\",\"content\":\"{\\\"completed_tool_checkpoint"
                )
            );
        }
        for sequence in 4..=7 {
            let record = invocation(&f, &session, &run, sequence).await;
            assert_eq!(
                record.result.unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .chars()
                    .count(),
                80
            );
        }
        assert_eq!(
            invocation(&f, &session, &run, 8).await.result.unwrap()["artifacts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}

#[tokio::test]
async fn relevant_skill_is_visible_before_discovery_and_checkpoint_handles_drive_next_steps() {
    let f = fixture(
        vec![
            tool(
                "search_capabilities",
                json!({"query":"document","kind":"tool"}),
            ),
            tool("read_skill", json!({"id":SKILL})),
            tool(
                "activate_tools",
                json!({"names":["workspace_create","workspace_edit","workspace_publish"]}),
            ),
            tool("workspace_create", json!({})),
            json!({"fixture_workspace_next":"workspace_edit"}),
            json!({"fixture_workspace_next":"workspace_publish"}),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start_prompt(&f, "Generate a Chinese document in the sandbox.").await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let search = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(search["related_skills"][0]["id"], SKILL);
    {
        let requests = f.upstream.requests.lock().unwrap();
        let system = requests[0]["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains(SKILL));
        assert!(!system.contains("Use sandbox_run, then publish only the final output file."));
        assert!(requests[1]["messages"].to_string().contains(SKILL));
        for request in requests.iter().skip(4) {
            assert!(
                !request
                    .to_string()
                    .contains("_persisted_arguments_reference")
            );
            for (index, message) in request["messages"].as_array().unwrap().iter().enumerate() {
                if let Some(calls) = message["tool_calls"].as_array() {
                    let function = &calls[0]["function"];
                    let arguments: Value =
                        serde_json::from_str(function["arguments"].as_str().unwrap()).unwrap();
                    if let Some(schema) = request["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|t| t["function"]["name"] == function["name"])
                    {
                        exoharness_tools::validate(&schema["function"]["parameters"],&arguments).expect("Historical tool arguments must remain schema-native, never checkpoint placeholders");
                    }
                    let next = &request["messages"][index + 1];
                    assert_eq!(next["role"], "tool");
                    assert_eq!(next["tool_call_id"], calls[0]["id"]);
                }
            }
        }
        assert!(
            !requests[5]["messages"]
                .to_string()
                .contains("fixture-large-layout")
        );
        assert!(requests[5]["messages"].as_array().unwrap().iter().any(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_str()
                    .unwrap()
                    .contains("completed_tool_checkpoint")
        }));
    }
    let workspace = invocation(&f, &session, &run, 4).await.result.unwrap();
    let edit = invocation(&f, &session, &run, 5).await;
    assert_eq!(
        edit.intent.arguments["workspace_id"],
        workspace["workspace_id"]
    );
    assert_eq!(edit.intent.arguments["expected_revision"], 1);
    let publish = invocation(&f, &session, &run, 6).await;
    assert_eq!(
        publish.intent.arguments["workspace_id"],
        workspace["workspace_id"]
    );
    assert_eq!(publish.intent.arguments["expected_revision"], 2);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn raw_mcp_name_activation_delivers_live_schema_and_exactly_one_execution() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("search_capabilities", json!({"query":"","kind":"tool"})),
            tool("activate_tools", json!({"names":["echo"]})),
            tool(&name, json!({"text":"retrieved data"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    assert_eq!(
        invocation(&f, &session, &run, 3).await.result.unwrap()["structuredContent"]["echo"]["text"],
        "retrieved data"
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
    assert!(
        f.upstream.requests.lock().unwrap()[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == name)
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn skill_discovery_exposes_native_interface_and_delivers_html_and_chart_in_real_oci() {
    let source = "from pathlib import Path\nPath('index.html').write_text('<!doctype html><html lang=\"zh\"><meta charset=\"UTF-8\"><title>测试网页</title><h1>文化旅游</h1></html>',encoding='utf-8')\nPath('chart.svg').write_text('<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"200\"><polyline points=\"20,160 100,140 200,90 300,30\" fill=\"none\" stroke=\"blue\"/></svg>')";
    let f = fixture(vec![
        tool("search_capabilities",json!({"query":"网页 绘图","kind":"tool"})),
        tool("read_skill",json!({"id":SKILL})),
        tool("activate_tools",json!({"names":["sandbox_run"]})),
        tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":source},"output_files":["index.html","chart.svg"]})),answer()
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let searched = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert!(
        searched["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["name"] == "sandbox_run")
    );
    let read = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(read["execution_guidance"]["workspace_available"], true);
    assert_eq!(
        read["execution_guidance"]["discover"]["arguments"],
        json!({"query":"","kind":"capability"})
    );
    let store = f.store.clone();
    let sid = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &sid, None, 8).unwrap()
    })
    .await
    .unwrap();
    let artifacts = detail["turns"][0]["response"]["artifacts"]
        .as_array()
        .unwrap();
    assert_eq!(artifacts.len(), 2);
    for artifact in artifacts {
        let url = url::Url::parse(artifact["download_url"].as_str().unwrap()).unwrap();
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
        assert!(
            !response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
