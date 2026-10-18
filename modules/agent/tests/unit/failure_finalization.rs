use super::*;
use exoharness_agent::finalize_tool_failure;

fn failed() -> EngineState {
    let mut s = state();
    s.blocked = Some(StopReason::ToolFailed);
    s.entries.push(JournalEntry::ToolExchange {
        invocation_id: "saved".into(),
        tool: "renderer".into(),
        arguments: json!({"saved_id":"plan-1"}),
        result: json!({"isError":true,"submitted":false,"phase":"metadata"}),
    });
    s
}
fn limits() -> EnginePolicy {
    let mut p = policy();
    p.max_reported_tokens = 300_000;
    p
}

#[tokio::test]
async fn one_tool_free_response_keeps_failed_state_and_never_replays_a_tool() {
    let mut s = failed();
    let mut d = Fixture::new(vec![completion(vec![])], vec![]);
    assert_eq!(
        finalize_tool_failure(&mut d, &mut s, &limits()).await,
        Some("done".into())
    );
    assert_eq!(s.blocked, Some(StopReason::ToolFailed));
    assert!(s.answer.is_none());
    assert!(d.calls.is_empty());
    assert_eq!(d.requests.len(), 1);
    assert!(d.requests[0].tools.is_empty());
    assert!(matches!(
        d.requests[0].tool_choice,
        exoharness_contracts::model::ToolChoice::None
    ));
    assert!(d.requests[0].output_token_limit <= 1024);
    let mut restored: EngineState =
        serde_json::from_value(serde_json::to_value(s).unwrap()).unwrap();
    assert!(
        finalize_tool_failure(&mut d, &mut restored, &limits())
            .await
            .is_none()
    );
    assert_eq!(d.requests.len(), 1);
}
#[tokio::test]
async fn token_exhaustion_missing_output_cap_cancel_and_permissions_do_not_call_model() {
    for case in [
        "budget",
        "output_cap",
        "cancel",
        "permission",
        "unknown",
        "save_failure",
    ] {
        let mut s = failed();
        let mut p = limits();
        let mut d = Fixture::new(vec![], vec![]);
        match case {
            "budget" => p.max_recovery_tokens = 1024,
            "output_cap" => p.recovery_output_cap_available = false,
            "cancel" => d.cancelled = true,
            "permission" => s.blocked = Some(StopReason::InputRejected),
            "unknown" => s.blocked = Some(StopReason::OutcomeUnknown),
            "save_failure" => d.fail_save = true,
            _ => unreachable!(),
        }
        assert!(
            finalize_tool_failure(&mut d, &mut s, &p).await.is_none(),
            "{case}"
        );
        assert!(d.requests.is_empty(), "{case}");
        assert!(d.calls.is_empty());
    }
}
#[tokio::test]
async fn proposed_tool_in_finalization_is_discarded_and_missing_usage_is_charged() {
    let mut s = failed();
    let mut c = completion(vec![proposal("must-not-dispatch")]);
    c.usage = Usage::default();
    let mut d = Fixture::new(vec![c], vec![]);
    assert!(
        finalize_tool_failure(&mut d, &mut s, &limits())
            .await
            .is_none()
    );
    assert!(s.recovery_tokens > 0 && s.recovery_tokens <= 16_384);
    assert!(d.calls.is_empty());
    assert!(s.pending.is_empty());
    assert!(
        finalize_tool_failure(&mut d, &mut s, &limits())
            .await
            .is_none()
    );
}
