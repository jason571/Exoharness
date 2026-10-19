use super::*;

fn program(id: &str, workspace: Option<&str>) -> ToolCall {
    let mut call = proposal(id);
    call.arguments = json!({"entrypoint":"build.py","runtime":"python"});
    if let Some(workspace) = workspace {
        call.arguments["workspace_id"] = json!(workspace);
    }
    call
}
fn failed_program() -> Receipt {
    let mut result = terminated_sandbox_failure("failed-program");
    result.result["workspace_id"] = json!("owned-workspace");
    result
}
fn completed_program() -> Receipt {
    let mut result = receipt("corrected-program", ReceiptState::Completed);
    result.result = json!({"isError":false,"workspace_id":"owned-workspace","execution":{"status":"completed","cleanup_confirmed":true,"exit_code":0},"files":[{"path":"output.pptx","sha256":"saved"}]});
    result
}

#[tokio::test]
async fn accepted_input_then_terminated_code_failure_transfers_only_its_episode() {
    let mut invalid_input = program("bad-input", None);
    invalid_input.arguments["runtime"] = json!("invalid-runtime");
    let mut edit = proposal("save-corrected-program");
    edit.arguments = json!({"workspace_id":"owned-workspace","patch":"corrected code"});
    let mut driver = Fixture::new(
        vec![
            completion(vec![invalid_input]),
            completion(vec![program("valid-input", None)]),
            completion(vec![edit]),
            completion(vec![program("fixed", Some("owned-workspace"))]),
            completion(vec![]),
        ],
        vec![
            input_rejection("invalid"),
            failed_program(),
            receipt("saved-code", ReceiptState::Completed),
            completed_program(),
        ],
    );
    let mut current = state();
    let mut p = policy();
    p.max_reported_tokens = 300_000;
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &p,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished("done".into())
    );
    assert!(!current.recovery_active);
    assert_eq!(current.input_corrections, 1);
    assert_eq!(current.sandbox_corrections, 1);
    assert!(current.input_recovery_targets.is_empty());
    assert!(current.sandbox_recovery_targets.is_empty());
    assert_eq!(current.recovery_tokens, 75);
}

#[tokio::test]
async fn confirmed_same_program_closes_only_its_repair_before_publication() {
    let mut edit = proposal("edit");
    edit.arguments = json!({"workspace_id":"owned-workspace","patch":"correct only palette"});
    let mut driver = Fixture::new(
        vec![
            completion(vec![program("bad", None)]),
            completion(vec![edit]),
            completion(vec![program("fixed", Some("owned-workspace"))]),
            completion(vec![]),
        ],
        vec![
            failed_program(),
            receipt("edit", ReceiptState::Completed),
            completed_program(),
        ],
    );
    let mut current = state();
    let mut p = policy();
    p.max_reported_tokens = 300_000;
    assert_eq!(
        run(
            &mut driver,
            &mut current,
            &p,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished("done".into())
    );
    assert!(!current.recovery_active);
    assert!(current.sandbox_recovery_targets.is_empty());
    assert_eq!(current.sandbox_corrections, 1);
    assert_eq!(current.recovery_tokens, 50); // edit and corrected-execution model proposals only
    assert_eq!(current.reported_tokens, 100);
    assert_eq!(driver.calls.len(), 3);
    assert_eq!(
        driver.requests.last().unwrap().output_token_limit,
        p.output_token_limit
    );
}

#[tokio::test]
async fn unrelated_or_unverified_success_cannot_close_a_sandbox_episode() {
    for mode in [
        "workspace",
        "entrypoint",
        "runtime",
        "local-read",
        "unconfirmed-cleanup",
        "missing-usage",
        "prior-recovery",
        "missing-exit",
    ] {
        let mut call = program("fixed", Some("owned-workspace"));
        let mut result = completed_program();
        let mut fixed = completion(vec![call.clone()]);
        let mut current = state();
        match mode {
            "workspace" => {
                call.arguments["workspace_id"] = json!("other");
                result.result["workspace_id"] = json!("other");
            }
            "entrypoint" => call.arguments["entrypoint"] = json!("other.py"),
            "runtime" => call.arguments["runtime"] = json!("node"),
            "local-read" => result.performed_execution = false,
            "unconfirmed-cleanup" => result.result["execution"]["cleanup_confirmed"] = json!(false),
            "missing-usage" => fixed.usage.input_tokens = None,
            "prior-recovery" => current.recovery_active = true,
            "missing-exit" => {
                result.result["execution"]
                    .as_object_mut()
                    .unwrap()
                    .remove("exit_code");
            }
            _ => unreachable!(),
        }
        fixed.calls = vec![call];
        let mut edit = proposal("save-corrected-program");
        edit.arguments = json!({"workspace_id":"owned-workspace","patch":"corrected code"});
        let mut driver = Fixture::new(
            vec![
                completion(vec![program("bad", None)]),
                completion(vec![edit]),
                fixed,
                completion(vec![]),
            ],
            vec![
                failed_program(),
                receipt("saved-code", ReceiptState::Completed),
                result,
            ],
        );
        let mut p = policy();
        p.max_reported_tokens = 300_000;
        run(
            &mut driver,
            &mut current,
            &p,
            "system",
            &BTreeSet::new(),
            &tools(),
        )
        .await;
        assert!(current.recovery_active, "{mode}");
        assert_eq!(current.sandbox_corrections, 1);
    }
}

#[tokio::test]
async fn restored_sandbox_target_is_exact_and_old_checkpoint_cannot_get_fresh_budget() {
    let mut current = state();
    current.recovery_active = true;
    current.sandbox_recovery_started = true;
    current.sandbox_corrections = 1;
    current.recovery_tokens = 37;
    current.sandbox_recovery_targets = vec![exoharness_agent::SandboxRecoveryTarget {
        tool: "registered-tool".into(),
        workspace: "owned-workspace".into(),
        entrypoint: "build.py".into(),
        runtime: "python".into(),
    }];
    let mut restored: EngineState =
        serde_json::from_value(serde_json::to_value(&current).unwrap()).unwrap();
    let mut driver = Fixture::new(
        vec![
            completion(vec![program("fixed", Some("owned-workspace"))]),
            completion(vec![]),
        ],
        vec![completed_program()],
    );
    let mut p = policy();
    p.max_reported_tokens = 300_000;
    run(
        &mut driver,
        &mut restored,
        &p,
        "system",
        &BTreeSet::new(),
        &tools(),
    )
    .await;
    assert!(!restored.recovery_active);
    assert_eq!(restored.recovery_tokens, 62);
    assert_eq!(restored.sandbox_corrections, 1);
    let mut legacy = serde_json::to_value(current).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("sandbox_recovery_started");
    legacy
        .as_object_mut()
        .unwrap()
        .remove("sandbox_recovery_targets");
    let mut restored: EngineState = serde_json::from_value(legacy).unwrap();
    let mut driver = Fixture::new(
        vec![
            completion(vec![program("fixed", Some("owned-workspace"))]),
            completion(vec![]),
        ],
        vec![completed_program()],
    );
    run(
        &mut driver,
        &mut restored,
        &p,
        "system",
        &BTreeSet::new(),
        &tools(),
    )
    .await;
    assert!(restored.recovery_active);
    assert!(restored.recovery_tokens > 37);
}
