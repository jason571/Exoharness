use super::*;
use exoharness_agent::project_request;

#[tokio::test]
async fn unchanged_invalid_arguments_stop_before_second_dispatch_or_further_models() {
    let mut driver = Fixture::new(
        vec![
            completion(vec![proposal("bad")]),
            completion(vec![proposal("unchanged")]),
        ],
        vec![input_rejection("rejected")],
    );
    let mut current = state();
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::UnchangedCorrection)
    );
    assert_eq!(driver.calls.len(), 1);
    assert_eq!(driver.requests.len(), 2);
    assert_eq!(current.input_corrections, 1);
    let mut restored: EngineState =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    assert_eq!(
        run(
            &mut driver,
            &mut restored,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::UnchangedCorrection)
    );
    assert_eq!(driver.requests.len(), 2);
}
#[tokio::test]
async fn recovery_budget_rejects_a_large_repair_request_before_provider_submission() {
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("bad")])],
        vec![input_rejection("rejected")],
    );
    let mut current = state();
    let mut limits = policy();
    limits.max_recovery_tokens = 1024;
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &limits,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
    );
    assert_eq!(driver.requests.len(), 1);
    assert_eq!(driver.calls.len(), 1);
    assert_eq!(current.recovery_tokens, 0);
}
#[tokio::test]
async fn recovery_output_cap_is_set_before_send_and_reported_input_and_output_are_charged() {
    let mut current = state();
    current.recovery_active = true;
    let original =
        project_request("system", &current.entries, &BTreeSet::new(), tools(), 500).unwrap();
    let bound = serde_json::to_vec(&original).unwrap().len() as u64 + 1024;
    let mut limits = policy();
    limits.requires_execution = false;
    limits.max_recovery_tokens = bound + 20;
    let mut driver = Fixture::new(vec![completion(vec![])], vec![]);
    assert!(matches!(
        run(
            &mut driver,
            &mut current,
            &limits,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(driver.requests[0].output_token_limit, 20);
    assert_eq!(current.recovery_tokens, 25);
    assert_eq!(current.recovery_token_limit, bound + 20);
}
#[tokio::test]
async fn missing_usage_keeps_the_full_reservation_and_cannot_buy_another_model_request() {
    let mut current = state();
    current.recovery_active = true;
    let original =
        project_request("system", &current.entries, &BTreeSet::new(), tools(), 500).unwrap();
    let bound = serde_json::to_vec(&original).unwrap().len() as u64 + 1024;
    let mut limits = policy();
    limits.max_recovery_tokens = bound + 500;
    let mut first = completion(vec![proposal("execute")]);
    first.usage.input_tokens = None;
    first.usage.output_tokens = None;
    let mut driver = Fixture::new(
        vec![first],
        vec![receipt("completed", ReceiptState::Completed)],
    );
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &limits,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
    );
    assert_eq!(driver.requests.len(), 1);
    assert_eq!(current.recovery_tokens, limits.max_recovery_tokens);
    assert_eq!(driver.calls.len(), 1);
}
#[tokio::test]
async fn restored_recovery_budget_is_cumulative_and_a_larger_policy_does_not_reset_it() {
    let mut current = state();
    current.recovery_active = true;
    current.recovery_token_limit = 1024;
    current.recovery_tokens = 1024;
    let mut restored: EngineState =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    let mut driver = Fixture::new(vec![], vec![]);
    assert_eq!(
        run(
            &mut driver,
            &mut restored,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
    );
    assert_eq!(restored.recovery_token_limit, 1024);
    assert!(driver.requests.is_empty());
}
#[tokio::test]
async fn legacy_recovery_without_attribution_does_not_receive_a_fresh_token_budget() {
    let mut old = serde_json::to_value(state()).unwrap();
    old["input_corrections"] = json!(1);
    for key in [
        "recovery_tokens",
        "recovery_token_limit",
        "recovery_active",
        "recovery_accounting_initialized",
    ] {
        old.as_object_mut().unwrap().remove(key);
    }
    let mut restored: EngineState = serde_json::from_value(old).unwrap();
    let mut driver = Fixture::new(vec![], vec![]);
    assert_eq!(
        run(
            &mut driver,
            &mut restored,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
    );
    assert_eq!(restored.recovery_tokens, policy().max_recovery_tokens);
    assert!(driver.requests.is_empty());
}

#[tokio::test]
async fn recovery_refuses_an_unbounded_provider_before_sending_a_model_request() {
    let mut current = state();
    current.recovery_active = true;
    let mut limits = policy();
    limits.recovery_output_cap_available = false;
    let mut driver = Fixture::new(vec![], vec![]);
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &limits,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
    );
    assert!(driver.requests.is_empty());
    assert_eq!(current.recovery_tokens, 0);
}

#[tokio::test]
async fn expanded_recovery_budget_accepts_a_realistic_schema_context_without_lifting_run_limits() {
    let mut current = state();
    current.recovery_active = true;
    let mut limits = policy();
    limits.requires_execution = false;
    limits.max_recovery_tokens = 196_608;
    let mut driver = Fixture::new(vec![completion(vec![])], vec![]);
    assert!(matches!(
        run(
            &mut driver,
            &mut current,
            &limits,
            &"s".repeat(40_000),
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(driver.requests.len(), 1);
    assert_eq!(current.recovery_tokens, 25);
    assert_eq!(current.recovery_token_limit, 196_608);
    assert!(driver.requests[0].output_token_limit <= limits.output_token_limit);
}

#[tokio::test]
async fn changing_only_revision_or_output_selection_does_not_retry_failed_sandbox_source() {
    let mut current = state();
    current.entries.push(JournalEntry::ToolExchange {
        invocation_id: "failed-source".into(),
        tool: "registered-tool".into(),
        arguments: json!({"workspace_id":"owned","expected_revision":2,"entrypoint":"main.py","runtime":"python","output_files":[]}),
        result: json!({"_host_sandbox_failure":1,"isError":true,"revision":3}),
    });
    let mut call = proposal("unused");
    call.arguments = json!({"workspace_id":"owned","expected_revision":3,"entrypoint":"main.py","runtime":"python","output_files":["final.pptx"]});
    let mut driver = Fixture::new(vec![completion(vec![call])], vec![]);
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::UnchangedCorrection)
    );
    assert!(driver.calls.is_empty());
    assert_eq!(driver.requests.len(), 1);
}
