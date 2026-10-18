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
