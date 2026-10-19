use super::*;
use exoharness_agent::{FailedModel, ModelCorrection};

fn failure(measured: bool, correction: Option<ModelCorrection>) -> FailedModel {
    FailedModel {
        usage: Usage {
            input_tokens: measured.then_some(12),
            output_tokens: measured.then_some(3),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        },
        correction,
    }
}

#[tokio::test]
async fn one_response_correction_does_not_charge_the_rest_of_a_healthy_task() {
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("build")]), completion(vec![])],
        vec![receipt("build", ReceiptState::Completed)],
    );
    driver
        .model_failures
        .insert(1, failure(true, Some(ModelCorrection::ToolNotLoaded)));
    let mut current = state();
    let mut limits = policy();
    // The corrected request fits. The later normal request contains the saved
    // tool result and would exceed this recovery reservation if misclassified.
    limits.max_recovery_tokens = 4_000;
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
        RunOutcome::Finished("done".into())
    );
    assert_eq!(current.recovery_tokens, 25);
    assert_eq!(current.reported_tokens, 65);
    assert!(!current.recovery_active);
    assert!(!current.model_correction_started_recovery);
    assert_eq!(current.model_corrections, 1);
}

#[tokio::test]
async fn model_correction_never_closes_preexisting_recovery_or_resets_its_usage() {
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("build")]), completion(vec![])],
        vec![receipt("build", ReceiptState::Completed)],
    );
    driver
        .model_failures
        .insert(1, failure(true, Some(ModelCorrection::ToolNotLoaded)));
    let mut current = state();
    current.recovery_active = true;
    current.recovery_tokens = 19;
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
        RunOutcome::Finished("done".into())
    );
    assert!(current.recovery_active);
    assert_eq!(current.recovery_tokens, 19 + 65);
}

#[tokio::test]
async fn unmeasured_replacement_keeps_the_reservation_and_recovery_active() {
    let mut replacement = completion(vec![proposal("build")]);
    replacement.usage.input_tokens = None;
    replacement.usage.output_tokens = None;
    let mut driver = Fixture::new(
        vec![replacement],
        vec![receipt("build", ReceiptState::Completed)],
    );
    driver
        .model_failures
        .insert(1, failure(true, Some(ModelCorrection::ToolNotLoaded)));
    let mut current = state();
    let mut limits = policy();
    limits.max_recovery_tokens = 4_000;
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
    assert!(current.recovery_active);
    assert!(!current.model_correction_started_recovery);
    assert!(current.recovery_tokens > 25);
    assert_eq!(driver.requests.len(), 2);
    assert_eq!(driver.calls.len(), 1);
}

#[tokio::test]
async fn subsequent_tool_recovery_reuses_the_accumulated_correction_allowance() {
    let mut corrected = proposal("fixed");
    corrected.arguments = json!({"native_array":[{"n":2}]});
    let mut driver = Fixture::new(
        vec![
            completion(vec![proposal("bad-input")]),
            completion(vec![corrected]),
            completion(vec![]),
        ],
        vec![
            input_rejection("bad-input"),
            receipt("fixed", ReceiptState::Completed),
        ],
    );
    driver
        .model_failures
        .insert(1, failure(true, Some(ModelCorrection::ToolNotLoaded)));
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
        RunOutcome::Finished("done".into())
    );
    assert_eq!(current.model_corrections, 1);
    assert_eq!(current.input_corrections, 1);
    assert!(!current.recovery_active);
    assert_eq!(current.recovery_tokens, 50);
    assert_eq!(current.reported_tokens, 90);
}

#[tokio::test]
async fn correction_recovery_ownership_survives_checkpoints_and_legacy_defaults_stay_conservative()
{
    for legacy in [false, true] {
        let mut current = state();
        current.model_corrections = 1;
        current.model_correction = Some(ModelCorrection::ToolNotLoaded);
        current.model_correction_started_recovery = true;
        current.recovery_active = true;
        current.recovery_tokens = 17;
        let mut saved = serde_json::to_value(current).unwrap();
        if legacy {
            saved
                .as_object_mut()
                .unwrap()
                .remove("model_correction_started_recovery");
        }
        let mut current = serde_json::from_value(saved).unwrap();
        let mut driver = Fixture::new(
            vec![completion(vec![proposal("build")]), completion(vec![])],
            vec![receipt("build", ReceiptState::Completed)],
        );
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
            RunOutcome::Finished("done".into())
        );
        assert_eq!(current.recovery_active, legacy);
        assert_eq!(current.recovery_tokens, 17 + if legacy { 50 } else { 25 });
    }
}

#[tokio::test]
async fn correction_after_completed_work_reuses_receipts_without_resubmitting_it() {
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("saved")]), completion(vec![])],
        vec![receipt("saved", ReceiptState::Completed)],
    );
    driver
        .model_failures
        .insert(2, failure(true, Some(ModelCorrection::ToolNotLoaded)));
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
        RunOutcome::Finished("done".into())
    );
    assert_eq!(driver.calls, vec!["model-1-tool-0"]);
    assert_eq!(driver.requests.len(), 3);
    assert!(
        serde_json::to_string(&driver.requests[2])
            .unwrap()
            .contains("registered-tool")
    );
    assert_eq!(current.reported_tokens, 65);
}
#[tokio::test]
async fn rejected_proposal_gets_one_diagnostic_led_correction_and_all_usage_is_counted() {
    for correction in [
        ModelCorrection::ToolArgumentsJson,
        ModelCorrection::ToolNotLoaded,
        ModelCorrection::CallIdReused,
        ModelCorrection::ArgumentsNotObject,
    ] {
        let mut driver = Fixture::new(
            vec![completion(vec![proposal("new")]), completion(vec![])],
            vec![receipt("new", ReceiptState::Completed)],
        );
        driver
            .model_failures
            .insert(1, failure(true, Some(correction)));
        let mut state = state();
        assert_eq!(
            run(
                &mut driver,
                &mut state,
                &policy(),
                "system",
                &BTreeSet::new(),
                &tools()
            )
            .await,
            RunOutcome::Finished("done".into())
        );
        assert_eq!(driver.requests.len(), 3);
        assert_eq!(driver.calls, vec!["model-2-tool-0"]);
        assert_eq!(state.reported_tokens, 65);
        assert_eq!(state.model_corrections, 1);
        assert!(state.model_correction.is_none());
        assert!(
            serde_json::to_string(&driver.requests[1])
                .unwrap()
                .contains("Host model-response diagnostic")
        );
        assert!(
            !serde_json::to_string(&driver.requests[2])
                .unwrap()
                .contains("Host model-response diagnostic")
        );
        assert_eq!(driver.notices[0]["submitted"], false);
    }
}
#[tokio::test]
async fn repeated_invalid_response_unknown_usage_and_noncorrectable_errors_stop() {
    for (measured, diagnostic, twice) in [
        (false, Some(ModelCorrection::ToolArgumentsJson), false),
        (true, None, false),
        (true, Some(ModelCorrection::ToolArgumentsJson), true),
    ] {
        let mut driver = Fixture::new(vec![], vec![]);
        driver
            .model_failures
            .insert(1, failure(measured, diagnostic));
        if twice {
            driver.model_failures.insert(2, failure(true, diagnostic));
        }
        let mut state = state();
        assert_eq!(
            run(
                &mut driver,
                &mut state,
                &policy(),
                "system",
                &BTreeSet::new(),
                &tools()
            )
            .await,
            RunOutcome::Stopped(StopReason::ModelIncomplete)
        );
        assert_eq!(driver.requests.len(), if twice { 2 } else { 1 });
        assert!(driver.calls.is_empty());
        if measured {
            assert_eq!(state.reported_tokens, if twice { 30 } else { 15 });
        }
    }
}
#[tokio::test]
async fn correction_respects_token_call_output_caps_and_restored_counters() {
    for variant in 0..5 {
        let mut driver = Fixture::new(vec![completion(vec![proposal("new")])], vec![]);
        driver
            .model_failures
            .insert(1, failure(true, Some(ModelCorrection::ToolArgumentsJson)));
        let mut state = state();
        let mut p = policy();
        match variant {
            0 => p.max_model_calls = 1,
            1 => p.max_reported_tokens = 15,
            2 => p.recovery_output_cap_available = false,
            3 => p.max_recovery_tokens = 1,
            _ => {
                state.model_corrections = 1;
                state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
            }
        }
        assert!(matches!(
            run(
                &mut driver,
                &mut state,
                &p,
                "system",
                &BTreeSet::new(),
                &tools()
            )
            .await,
            RunOutcome::Stopped(_)
        ));
        assert_eq!(driver.requests.len(), 1);
        assert!(driver.calls.is_empty());
    }
}
#[tokio::test]
async fn large_discovery_metadata_does_not_enter_cursor_or_change_business_records() {
    let mut driver = Fixture::new(vec![completion(vec![])], vec![]);
    driver.initial_discovery = Some(exoharness_agent::InitialDiscovery::new(
        json!({"items":[{"kind":"capability","id":"large","name":"Large","revision":1,"description":"RECOGNITION ONLY ".repeat(20_000)}]}),
        vec![],
    ));
    let mut state = state();
    let mut p = policy();
    p.max_context_bytes = 1_000_000;
    p.requires_execution = false;
    assert_eq!(
        run(&mut driver, &mut state, &p, "system", &BTreeSet::new(), &[]).await,
        RunOutcome::Finished("done".into())
    );
    let saved = driver.saved.lock().unwrap().clone().unwrap();
    let encoded = serde_json::to_string(&saved).unwrap();
    assert!(encoded.len() < 64 * 1024);
    assert!(!encoded.contains("RECOGNITION ONLY"));
    assert!(
        serde_json::to_string(&driver.requests[0])
            .unwrap()
            .contains("RECOGNITION ONLY")
    );
}
