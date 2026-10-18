//! Opt-in private journal replay; never calls a provider or executes a tool.
use exoharness_agent::{project_request, workflow_view};
use exoharness_contracts::model::{ChatRequest, Message};
use exoharness_core::context::JournalEntry;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Recorded {
    entries: Vec<JournalEntry>,
    request: ChatRequest,
}

#[test]
#[ignore = "requires private MV projection fixture; no model or media execution"]
fn replay_mv_stages_with_declared_skill_context() {
    let path = std::env::var("EXOHARNESS_MV_CONTEXT_REPLAY").expect("explicit fixture required");
    let recorded: Vec<Recorded> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut total_before = 0;
    let mut total_after = 0;
    for (index, sample) in recorded.iter().enumerate() {
        let original = sample.entries.clone();
        let Message::System { text: system } = &sample.request.messages[0] else {
            panic!("system required")
        };
        let stage = workflow_view(&sample.entries)
            .map(|v| v.tools)
            .or_else(|| exoharness_agent::initial_skill_tools(&sample.entries).map(|v| v.0));
        let tools = if let Some(stage) = stage {
            sample
                .request
                .tools
                .iter()
                .filter(|t| {
                    stage.contains(&t.name)
                        || matches!(
                            t.name.as_str(),
                            "search_capabilities" | "activate_tools" | "read_skill"
                        )
                })
                .cloned()
                .collect()
        } else {
            sample.request.tools.clone()
        };
        let projected = project_request(
            system,
            &sample.entries,
            &BTreeSet::new(),
            tools,
            sample.request.output_token_limit,
        )
        .unwrap();
        if index == 5
            && let Ok(path) = std::env::var("EXOHARNESS_MV_PLANNING_REQUEST")
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .unwrap();
            std::io::Write::write_all(&mut file, &serde_json::to_vec(&projected).unwrap()).unwrap();
        }
        let before = serde_json::to_vec(&sample.request).unwrap().len();
        let after = serde_json::to_vec(&projected).unwrap().len();
        println!(
            "MV request {}: bytes {} -> {}, schemas {} -> {}",
            index + 1,
            before,
            after,
            sample.request.tools.len(),
            projected.tools.len()
        );
        total_before += before;
        total_after += after;
        assert_eq!(sample.entries, original);
        for (i, message) in projected.messages.iter().enumerate() {
            if let Message::Assistant { calls, .. } = message
                && !calls.is_empty()
            {
                assert!(
                    matches!(&projected.messages[i+1],Message::ToolResult{call_id,..} if *call_id==calls[0].id)
                );
            }
        }
        if index == 3 || index == 4 {
            assert_eq!(projected.tools.len(), 4);
            assert!(after < before * 3 / 4);
        }
    }
    assert!(total_after < total_before * 85 / 100);
    println!(
        "MV cumulative projected bytes: {total_before} -> {total_after}; not measured provider tokens or media quality"
    );
}

#[test]
#[ignore = "requires a private exported journal fixture with adapter-verified checkpoint annotations"]
fn replay_real_requests_without_model_or_tool_execution() {
    let path =
        std::env::var("EXOHARNESS_CONTEXT_REPLAY").expect("explicit private fixture required");
    let recorded: Vec<Recorded> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut before_total = 0;
    let mut after_total = 0;
    for (index, sample) in recorded.iter().enumerate() {
        let Message::System { text: system } = &sample.request.messages[0] else {
            panic!("system required")
        };
        let view = workflow_view(&sample.entries);
        let tools = if let Some(view) = view.as_ref() {
            sample
                .request
                .tools
                .iter()
                .filter(|tool| {
                    view.tools.contains(&tool.name)
                        || matches!(
                            tool.name.as_str(),
                            "search_capabilities" | "activate_tools" | "read_skill"
                        )
                })
                .cloned()
                .collect()
        } else {
            sample.request.tools.clone()
        };
        let projected = project_request(
            system,
            &sample.entries,
            &BTreeSet::new(),
            tools,
            sample.request.output_token_limit,
        )
        .unwrap();
        let before = serde_json::to_vec(&sample.request).unwrap().len();
        let after = serde_json::to_vec(&projected).unwrap().len();
        before_total += before;
        after_total += after;
        println!(
            "request {}: bytes {} -> {}, schemas {} -> {}",
            index + 1,
            before,
            after,
            sample.request.tools.len(),
            projected.tools.len()
        );
        if index == recorded.len() - 1 {
            assert!(view.as_ref().is_some_and(|v| !v.required));
            assert!(
                after < before / 2,
                "final delivery must eliminate substantial repeated input"
            );
        }
    }
    println!(
        "cumulative request bytes: {before_total} -> {after_total}; this is not a provider token or price measurement"
    );
    assert!(after_total < before_total);
}

#[test]
#[ignore = "requires private search-to-video request capture; no external execution"]
fn replay_large_search_to_video_requests_losslessly() {
    #[derive(Deserialize)]
    struct Sample {
        sequence: u64,
        request: ChatRequest,
    }
    let path = std::env::var("EXOHARNESS_SEARCH_VIDEO_REPLAY").expect("explicit fixture required");
    let samples: Vec<Sample> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(samples.len(), 10);
    let mut before_total = 0;
    let mut after_total = 0;
    for sample in samples {
        let mut system = String::new();
        let mut entries = Vec::new();
        let mut pending = None;
        for message in &sample.request.messages {
            match message {
                Message::System { text } => system = text.clone(),
                Message::User { text } => entries.push(JournalEntry::User(text.clone())),
                Message::Assistant { text, calls } if calls.is_empty() => {
                    entries.push(JournalEntry::Assistant(text.clone()))
                }
                Message::Assistant { calls, .. } => {
                    assert_eq!(calls.len(), 1);
                    pending = Some(calls[0].clone());
                }
                Message::ToolResult {
                    call_id, content, ..
                } => {
                    let call = pending.take().expect("paired original call");
                    assert_eq!(call.id, *call_id);
                    entries.push(JournalEntry::ToolExchange {
                        invocation_id: call.id,
                        tool: call.name,
                        arguments: call.arguments,
                        result: serde_json::from_str(content).unwrap(),
                    });
                }
                _ => panic!("unexpected original message variant"),
            }
        }
        let original = entries.clone();
        let projected = project_request(
            &system,
            &entries,
            &BTreeSet::new(),
            sample.request.tools.clone(),
            sample.request.output_token_limit,
        )
        .unwrap();
        assert_eq!(entries, original);
        assert_eq!(serde_json::to_value(&projected.tools).unwrap(), serde_json::to_value(&sample.request.tools).unwrap());
        for (index, message) in projected.messages.iter().enumerate() {
            if let Message::Assistant { calls, .. } = message
                && !calls.is_empty()
            {
                assert!(
                    matches!(&projected.messages[index+1],Message::ToolResult{call_id,..} if *call_id==calls[0].id)
                );
            }
        }
        let before = serde_json::to_vec(&sample.request).unwrap().len();
        let after = serde_json::to_vec(&projected).unwrap().len();
        before_total += before;
        after_total += after;
        println!(
            "search/video request {} bytes {} -> {}",
            sample.sequence, before, after
        );
        if sample.sequence == 10 {
            assert!(
                projected
                    .tools
                    .iter()
                    .any(|tool| tool.name == "mcp_generate_video_af4b82f52233c54c")
            );
            assert!(
                projected
                    .tools
                    .iter()
                    .any(|tool| tool.name == "mcp_list_video_models_082b88a33503966e")
            );
        }
    }
    assert!(after_total < before_total * 7 / 10);
    println!(
        "cumulative exact request bytes: {before_total} -> {after_total}; no provider tokens billed or estimated"
    );
}
