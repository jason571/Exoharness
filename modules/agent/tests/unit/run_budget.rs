use super::*;

#[tokio::test]
async fn measured_budget_guidance_preserves_full_receipts_and_executable_tools() {
    for (spent, expected) in [
        (29_999, None),
        (30_000, Some("Reserve most")),
        (100_000, Some("substantial part")),
        (200_000, Some("Prioritize completing")),
    ] {
        let mut state = EngineState::new(vec![JournalEntry::User("Create the requested presentation".into()), JournalEntry::ToolExchange {
            invocation_id: "source".into(), tool: "fetch".into(), arguments: json!({"source":"official"}),
            result: json!({"period":"last confirmed quarter","facts":[1,null,{"table":[["value",4.7]]}],"code":"print('complete source')"}),
        }], 3).unwrap();
        state.reported_tokens = spent;
        state.usage.input_tokens = Some(spent);
        let original = serde_json::to_value(&state.entries).unwrap();
        let mut f = Fixture::new(vec![completion(vec![])], vec![]);
        let mut p = policy();
        p.max_reported_tokens = 300_000;
        p.requires_execution = false;
        assert_eq!(
            run(&mut f, &mut state, &p, "system", &BTreeSet::new(), &tools()).await,
            RunOutcome::Finished("done".into())
        );
        let request = &f.requests[0];
        let guidance = request.messages.iter().find_map(|message| match message {
            Message::RuntimeData { text } if text.contains("host_run_budget") => Some(text),
            _ => None,
        });
        match expected {
            Some(expected) => {
                let text = guidance.unwrap();
                assert!(text.contains(expected));
                let value: serde_json::Value = serde_json::from_str(text).unwrap();
                assert_eq!(value["host_run_budget"]["reported_tokens_used"], spent);
                assert_eq!(
                    value["host_run_budget"]["reported_tokens_remaining"],
                    300_000 - spent
                );
            }
            None => assert!(guidance.is_none()),
        }
        assert_eq!(request.tools.len(), tools().len());
        assert_eq!(serde_json::to_value(&state.entries[..2]).unwrap(), original);
        assert!(request.messages.iter().any(|m| matches!(m,Message::ToolResult { content,.. } if content.contains("complete source") && content.contains("last confirmed quarter"))));
    }
}

#[tokio::test]
async fn incomplete_usage_is_marked_and_restored_budget_does_not_reset() {
    let mut state = EngineState::new(vec![JournalEntry::User("create".into())], 3).unwrap();
    state.reported_tokens = 100_000;
    state.unreported_requests = 2;
    state.usage.input_tokens = None;
    let mut state: EngineState =
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    let mut p = policy();
    p.max_reported_tokens = 300_000;
    p.requires_execution = false;
    let mut f = Fixture::new(vec![completion(vec![])], vec![]);
    run(&mut f, &mut state, &p, "system", &BTreeSet::new(), &tools()).await;
    let guidance = f.requests[0]
        .messages
        .iter()
        .find_map(|m| match m {
            Message::RuntimeData { text } if text.contains("host_run_budget") => {
                Some(serde_json::from_str::<serde_json::Value>(text).unwrap())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(guidance["host_run_budget"]["usage_complete"], false);
    assert_eq!(guidance["host_run_budget"]["unreported_requests"], 2);
    assert!(state.reported_tokens >= 100_000);
}
