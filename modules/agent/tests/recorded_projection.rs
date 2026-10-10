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
