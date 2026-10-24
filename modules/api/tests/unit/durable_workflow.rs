use super::*;

#[tokio::test]
async fn verified_skill_opt_in_loads_entry_schema_without_discovery_or_activation_rounds() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let f = fixture(
        vec![
            tool("read_skill", json!({"id":SKILL})),
            tool(&prepare, json!({"text":"saved plan"})),
            tool(&finish, json!({"state_id":"saved-1"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (skills, registry) = (f.tools.skills.clone(), f.tools.registry.clone());
    tokio::task::spawn_blocking(move||{
        let root="---\nname: document-fixture\ndescription: Fixture workflow.\n---\nKeep the user's requirements.\n## Planning\nPlan once.\n## Delivery\nKeep exact audio.\n";
        let declaration=json!({"version":1,"auto_activate_entry":true,"entry_tools":["echo"],"stages":[{"tools":["echo"],"omit_sections":[]},{"tools":["other"],"omit_sections":["## Planning"]}]});
        let upload=serde_json::from_value(json!({"expected_revision":registry.0.capability(&RegistryAccess::Administrator,SKILL).unwrap().revision,"files":[{"path":"SKILL.md","content_base64":STANDARD.encode(root)},{"path":"context-stages.json","content_base64":STANDARD.encode(declaration.to_string())}]})).unwrap();
        let record=skills.import(&RegistryAccess::Administrator,&upload,"admin").unwrap();
        registry.0.set_capability_enabled(&RegistryAccess::Administrator,&record.id,record.revision,true,"admin").unwrap();
    }).await.unwrap();
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let original = invocation(&f, &session, &run, 1).await;
    assert_eq!(
        original.result.unwrap()["_host_skill_context"]["auto_activate_entry"],
        true
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let names: Vec<_> = requests[1]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&prepare.as_str()));
        assert!(!names.contains(&finish.as_str()));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn skill_declared_entry_and_render_context_shrink_without_losing_global_rules() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let f = fixture(
        vec![
            tool("read_skill", json!({"id":SKILL})),
            tool("activate_tools", json!({"names":[prepare,finish]})),
            tool(&prepare, json!({"text":"exact authored plan"})),
            tool(&finish, json!({"text":"consume saved plan"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let root="---\nname: document-fixture\ndescription: Fixture workflow.\n---\nGLOBAL CULTURE AND FINAL DELIVERY\n## Planning\n".to_string()+&"PLANNING ONLY INSTRUCTIONS\n".repeat(500)+"## Rendering\nKEEP EXACT ORIGINAL AUDIO\n";
    let declaration = json!({"version":1,"entry_tools":["echo"],"stages":[{"tools":["echo"],"omit_sections":[]},{"tools":["other"],"omit_sections":["## Planning"]}]});
    let (skills, registry) = (f.tools.skills.clone(), f.tools.registry.clone());
    let original = root.clone();
    tokio::task::spawn_blocking(move || {
        let upload=serde_json::from_value(json!({"expected_revision":registry.0.capability(&RegistryAccess::Administrator,SKILL).unwrap().revision,"files":[{"path":"SKILL.md","content_base64":STANDARD.encode(original)},{"path":"context-stages.json","content_base64":STANDARD.encode(declaration.to_string())}]})).unwrap();
        let record=skills.import(&RegistryAccess::Administrator,&upload,"admin").unwrap();
        registry.0.set_capability_enabled(&RegistryAccess::Administrator,&record.id,record.revision,true,"admin").unwrap();
    }).await.unwrap();
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&prepare.as_str()));
        assert!(!names.contains(&finish.as_str()));
        let render = requests[3].to_string();
        assert!(render.contains("GLOBAL CULTURE AND FINAL DELIVERY"));
        assert!(render.contains("KEEP EXACT ORIGINAL AUDIO"));
        assert!(!render.contains("PLANNING ONLY INSTRUCTIONS"));
        assert!(
            !requests[4]
                .to_string()
                .contains("PLANNING ONLY INSTRUCTIONS")
        );
    }
    assert_eq!(
        invocation(&f, &session, &run, 1).await.result.unwrap()["content"],
        root
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn durable_mcp_stages_shrink_context_and_schemas_without_mutating_exact_journal() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let source = "EXACT AUTHORED PLAN".repeat(1800);
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[prepare]})),
            tool(&prepare, json!({"text":source})),
            tool(&finish, json!({"state_id":"saved-1"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let receipt = invocation(&f, &session, &run, 2).await;
    assert_eq!(receipt.intent.arguments["text"], source);
    assert!(
        receipt.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("OLD FULL RESULT")
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let stage = requests[2].to_string();
        assert!(!stage.contains("EXACT AUTHORED PLAN"));
        assert!(!stage.contains("OLD FULL RESULT"));
        assert!(stage.contains("retained input"));
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&finish.as_str()));
        assert!(!names.contains(&prepare.as_str()));
        let final_request = requests[3].to_string();
        assert!(final_request.contains("KEEP COMPLETE FACT"));
        assert!(final_request.contains("final.mp4"));
        assert!(!final_request.contains("OLD FULL RESULT"));
        let names: Vec<_> = requests[3]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&finish.as_str()));
        assert!(!names.contains(&prepare.as_str()));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn missing_declared_checkpoint_value_keeps_exact_result_and_normal_tools() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        workflow_missing_preserve: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[prepare,finish]})),
            tool(&prepare, json!({"text":"exact input"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert!(requests[2].to_string().contains("OLD FULL RESULT"));
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&prepare.as_str()));
        assert!(names.contains(&finish.as_str()));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn premature_schema_activation_keeps_prerequisites_until_committed_checkpoint() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let f = fixture(
        vec![
            tool("read_skill", json!({"id":SKILL})),
            tool("read_skill", json!({"id":SKILL,"path":"notes.md"})),
            tool("activate_tools", json!({"names":[finish]})),
            tool("activate_tools", json!({"names":[prepare]})),
            tool(&prepare, json!({"text":"exact saved plan"})),
            tool(&finish, json!({"state_id":"saved-1"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (skills, registry) = (f.tools.skills.clone(), f.tools.registry.clone());
    tokio::task::spawn_blocking(move || {
        let root = "---\nname: document-fixture\ndescription: Fixture workflow.\n---\nGLOBAL DELIVERY RULES\n## Planning\nMUST SAVE BEFORE NEXT STAGE\n## Delivery\nUSE EXISTING INPUTS\n";
        let declaration = json!({"version":1,"auto_activate_entry":true,"entry_tools":["echo"],"stages":[{"tools":["echo"],"omit_sections":[]},{"tools":["other"],"omit_sections":["## Planning"]}]});
        let upload = serde_json::from_value(json!({"expected_revision":registry.0.capability(&RegistryAccess::Administrator,SKILL).unwrap().revision,"files":[{"path":"SKILL.md","content_base64":STANDARD.encode(root)},{"path":"context-stages.json","content_base64":STANDARD.encode(declaration.to_string())},{"path":"notes.md","content_base64":STANDARD.encode("Additional reference.")}]})).unwrap();
        let record = skills.import(&RegistryAccess::Administrator, &upload, "admin").unwrap();
        registry.0.set_capability_enabled(&RegistryAccess::Administrator,&record.id,record.revision,true,"admin").unwrap();
    }).await.unwrap();
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 7);
        let premature = &requests[3];
        assert!(
            premature["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == finish)
        );
        assert!(
            premature["messages"]
                .to_string()
                .contains("MUST SAVE BEFORE NEXT STAGE")
        );
        let committed = requests[5]["messages"].to_string();
        assert!(!committed.contains("MUST SAVE BEFORE NEXT STAGE"));
        assert!(committed.contains("GLOBAL DELIVERY RULES"));
        assert!(committed.contains("USE EXISTING INPUTS"));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

async fn resource_stage_fixture(reread: bool) -> (Fixture, mcp_server::Server) {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let mut replies = vec![
        tool("read_skill", json!({"id":SKILL})),
        tool("read_skill", json!({"id":SKILL,"path":"authoring.md"})),
        tool(&prepare, json!({"text":"saved authored input"})),
    ];
    if reread {
        replies.push(tool(
            "read_skill",
            json!({"id":SKILL,"path":"authoring.md"}),
        ));
    }
    replies.extend([tool(&finish, json!({"state_id":"saved-1"})), answer()]);
    let f = fixture(replies, true).await;
    register_mcp(&f, &remote.url).await;
    let (skills, registry) = (f.tools.skills.clone(), f.tools.registry.clone());
    tokio::task::spawn_blocking(move || {
        let root="---\nname: document-fixture\ndescription: Fixture workflow.\n---\nGLOBAL DELIVERY REQUIREMENTS\n## Planning\nPREPARATION INSTRUCTIONS\n## Finish\nKEEP FINAL CONTRACT\n";
        let declaration=json!({"version":1,"auto_activate_entry":true,"entry_tools":["echo"],"stages":[{"tools":["echo"],"omit_sections":[]},{"tools":["other"],"omit_sections":["## Planning"],"omit_resources":["authoring.md"]}]});
        let upload=serde_json::from_value(json!({"expected_revision":registry.0.capability(&RegistryAccess::Administrator,SKILL).unwrap().revision,"files":[{"path":"SKILL.md","content_base64":STANDARD.encode(root)},{"path":"context-stages.json","content_base64":STANDARD.encode(declaration.to_string())},{"path":"authoring.md","content_base64":STANDARD.encode("EXACT IMMUTABLE AUTHORING REFERENCE\n".repeat(200))}]})).unwrap();
        let record=skills.import(&RegistryAccess::Administrator,&upload,"admin").unwrap();
        registry.0.set_capability_enabled(&RegistryAccess::Administrator,&record.id,record.revision,true,"admin").unwrap();
    }).await.unwrap();
    (f, remote)
}

#[tokio::test]
async fn skill_reference_read_keeps_ready_entry_and_retires_only_after_checkpoint() {
    let (f, _remote) = resource_stage_fixture(false).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 5);
        let before = requests[2].to_string();
        assert!(before.contains("EXACT IMMUTABLE AUTHORING REFERENCE"));
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 4);
        assert!(names.iter().any(|name| name.starts_with("mcp_echo_")));
        let after = requests[3].to_string();
        assert!(!after.contains("EXACT IMMUTABLE AUTHORING REFERENCE"));
        assert!(after.contains("skill_resource_projection"));
        assert!(after.contains("GLOBAL DELIVERY REQUIREMENTS"));
        assert!(after.contains("KEEP FINAL CONTRACT"));
    }
    let original = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert!(
        original["content"]
            .as_str()
            .unwrap()
            .contains("EXACT IMMUTABLE AUTHORING REFERENCE")
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn explicit_cached_reference_read_restores_authoring_content_after_checkpoint() {
    let (f, _remote) = resource_stage_fixture(true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 6);
        assert!(
            !requests[3]
                .to_string()
                .contains("EXACT IMMUTABLE AUTHORING REFERENCE")
        );
        assert!(
            requests[4]
                .to_string()
                .contains("EXACT IMMUTABLE AUTHORING REFERENCE")
        );
    }
    assert_eq!(
        invocation(&f, &session, &run, 4).await.result.unwrap()["already_loaded"],
        true
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
