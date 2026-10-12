use super::*;

#[allow(clippy::too_many_arguments)]
fn saved_read(
    current: &mut EngineState,
    id: &str,
    path: &str,
    hash: &str,
    revision: u64,
    start: usize,
    text: &str,
    next: serde_json::Value,
    verified: bool,
) {
    let raw = json!({"workspace_id":"owned","revision":revision,"file":{"path":path,"sha256":hash},"offset":start,"next_offset":next,"content":text});
    current.entries.push(JournalEntry::ToolExchange {
        invocation_id: id.into(),
        tool: "registered-tool".into(),
        arguments: json!({"workspace_id":"owned","path":path}),
        result: raw.clone(),
    });
    if verified {
        let mut summary = raw;
        summary["_durable_read_body"] = json!(true);
        current.entries.push(JournalEntry::Checkpoint {
            invocation_id: id.into(),
            reference: format!("store:{id}"),
            summary,
        });
    }
}
fn seeded() -> EngineState {
    let mut current = state();
    for n in 0..2 {
        for path in ["a", "b", "c"] {
            saved_read(
                &mut current,
                &format!("{path}{n}"),
                path,
                &"a".repeat(64),
                4,
                0,
                "source",
                serde_json::Value::Null,
                true,
            );
        }
    }
    current
}
fn reading(path: &str) -> ToolCall {
    let mut call = proposal(path);
    call.arguments = json!({"workspace_id":"owned","path":path,"offset":0,"limit":16000});
    call
}
#[tokio::test]
async fn alternating_reads_are_reviewed_before_dispatch_and_model_can_finish_with_a_real_action() {
    let mut current = seeded();
    let mut driver = Fixture::new(
        vec![
            completion(vec![reading("a"), proposal("stale-dependent")]),
            completion(vec![proposal("publish")]),
            completion(vec![]),
        ],
        vec![receipt("published", ReceiptState::Completed)],
    );
    assert!(matches!(
        run(
            &mut driver,
            &mut current,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(driver.calls.len(), 1);
    assert_eq!(driver.requests.len(), 3);
    assert_eq!(driver.notices.len(), 1);
    assert_eq!(driver.notices[0]["proposal_not_submitted"], true);
    assert_eq!(current.read_loop_reviews.len(), 1);
    assert_eq!(current.recovery_tokens, 50);
    assert!(
        serde_json::to_string(&driver.requests[1])
            .unwrap()
            .contains("Host read-loop review")
    );
    let request_json = serde_json::to_string(&driver.requests[1]).unwrap();
    assert!(request_json.contains("host_read_loop_reviews"));
    assert!(request_json.contains("store:a0"));
    let restored: EngineState =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    assert_eq!(restored.read_loop_reviews, current.read_loop_reviews);
}
#[tokio::test]
async fn two_reviews_then_one_tool_free_finalization_bound_an_uncooperative_model() {
    let mut current = seeded();
    current.successful_executions = 1;
    let mut driver = Fixture::new(
        vec![
            completion(vec![reading("a")]),
            completion(vec![reading("b")]),
            completion(vec![reading("c")]),
            completion(vec![]),
        ],
        vec![],
    );
    assert!(matches!(
        run(
            &mut driver,
            &mut current,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert!(driver.calls.is_empty());
    assert_eq!(current.read_loop_reviews.len(), 2);
    assert!(current.repeat_finalization);
    assert!(driver.requests[3].tools.is_empty());
    assert_eq!(driver.notices.len(), 3);
    assert_eq!(driver.notices[2]["phase"], "finalize");
    let mut restored: EngineState =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    let before = driver.requests.len();
    assert!(matches!(
        run(
            &mut driver,
            &mut restored,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(driver.requests.len(), before);
}
#[tokio::test]
async fn changed_revision_hash_or_read_range_and_unverified_receipts_are_not_blocked() {
    for variant in ["revision", "hash", "range", "unverified", "tool"] {
        let mut current = state();
        for n in 0..2 {
            saved_read(
                &mut current,
                &format!("read{n}"),
                "a",
                &"a".repeat(64),
                4,
                0,
                "source",
                serde_json::Value::Null,
                variant != "unverified",
            );
        }
        if variant == "revision" || variant == "hash" {
            let result = json!({"workspace_id":"owned","revision":5,"files":[{"path":"a","sha256":"b".repeat(64)}]});
            current.entries.push(JournalEntry::ToolExchange {
                invocation_id: "edit".into(),
                tool: "edit".into(),
                arguments: json!({}),
                result: result.clone(),
            });
            current.entries.push(JournalEntry::Checkpoint {
                invocation_id: "edit".into(),
                reference: "store:edit".into(),
                summary: result,
            });
        }
        if variant == "tool" {
            for e in &mut current.entries {
                if let JournalEntry::ToolExchange { tool, .. } = e {
                    *tool = "other".into();
                }
            }
        }
        let mut call = reading("a");
        if variant == "range" {
            call.arguments["offset"] = json!(2);
            call.arguments["limit"] = json!(2);
        }
        let mut driver = Fixture::new(
            vec![completion(vec![call]), completion(vec![])],
            vec![receipt("new", ReceiptState::Completed)],
        );
        assert!(
            matches!(
                run(
                    &mut driver,
                    &mut current,
                    &policy(),
                    "system",
                    &BTreeSet::new(),
                    &tools()
                )
                .await,
                RunOutcome::Finished(_)
            ),
            "{variant}"
        );
        assert_eq!(driver.calls.len(), 1, "{variant}");
        assert!(driver.notices.is_empty(), "{variant}");
    }
}
#[tokio::test]
async fn unicode_query_and_default_offsets_are_normalized_from_confirmed_file_content() {
    let mut current = state();
    for n in 0..2 {
        saved_read(
            &mut current,
            &format!("r{n}"),
            "a",
            &"a".repeat(64),
            4,
            0,
            "中文source",
            serde_json::Value::Null,
            true,
        );
    }
    let mut call = reading("a");
    call.arguments.as_object_mut().unwrap().remove("offset");
    call.arguments["query"] = json!("中文");
    let mut limits = policy();
    limits.requires_execution = false;
    let mut driver = Fixture::new(vec![completion(vec![call]), completion(vec![])], vec![]);
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
    assert!(driver.calls.is_empty());
    assert_eq!(driver.notices[0]["review"]["length"], 8);
}

#[tokio::test]
async fn unrelated_workspace_revision_does_not_reset_the_same_file_hash_but_a_new_turn_does() {
    for new_turn in [false, true] {
        let mut current = seeded();
        let result = json!({"workspace_id":"owned","revision":5,"files":[{"path":"a","sha256":"a".repeat(64)},{"path":"other","sha256":"b".repeat(64)}]});
        current.entries.push(JournalEntry::ToolExchange {
            invocation_id: "unrelated-edit".into(),
            tool: "edit".into(),
            arguments: json!({}),
            result: result.clone(),
        });
        current.entries.push(JournalEntry::Checkpoint {
            invocation_id: "unrelated-edit".into(),
            reference: "store:edit".into(),
            summary: result,
        });
        if new_turn {
            current
                .entries
                .push(JournalEntry::User("Please examine this file again".into()));
        }
        let mut limits = policy();
        limits.requires_execution = false;
        let receipts = if new_turn {
            vec![receipt("fresh", ReceiptState::Completed)]
        } else {
            vec![]
        };
        let mut driver = Fixture::new(
            vec![completion(vec![reading("a")]), completion(vec![])],
            receipts,
        );
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
        assert_eq!(driver.notices.len(), usize::from(!new_turn));
        assert_eq!(driver.calls.len(), usize::from(new_turn));
    }
}

#[tokio::test]
async fn notice_persistence_failure_and_cancellation_never_dispatch_a_reviewed_read() {
    for cancelled in [false, true] {
        let mut current = seeded();
        let mut driver = Fixture::new(vec![completion(vec![reading("a")])], vec![]);
        driver.cancelled = cancelled;
        driver.fail_notice = !cancelled;
        let reason = if cancelled {
            StopReason::Cancelled
        } else {
            StopReason::PersistenceFailed
        };
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
            RunOutcome::Stopped(reason.clone())
        );
        assert!(driver.calls.is_empty());
        assert_eq!(driver.requests.len(), usize::from(!cancelled));
        let mut restored: EngineState =
            serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
        driver.fail_notice = false;
        driver.cancelled = false;
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
            RunOutcome::Stopped(reason)
        );
        assert_eq!(driver.requests.len(), usize::from(!cancelled));
    }
}

#[tokio::test]
async fn legacy_checkpoint_keeps_historical_read_counts_and_context_limit_stays_enforced() {
    let mut value = serde_json::to_value(seeded()).unwrap();
    value.as_object_mut().unwrap().remove("read_loop_reviews");
    let mut current: EngineState = serde_json::from_value(value).unwrap();
    let mut limits = policy();
    limits.requires_execution = false;
    let mut driver = Fixture::new(
        vec![completion(vec![reading("a")]), completion(vec![])],
        vec![],
    );
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
    assert_eq!(current.read_loop_reviews.len(), 1);
    assert!(driver.calls.is_empty());
    let mut current = seeded();
    let mut limits = policy();
    limits.max_context_bytes = 1;
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
        RunOutcome::Stopped(StopReason::ContextBudget)
    );
    assert!(driver.requests.is_empty());
}

#[tokio::test]
async fn source_changed_after_a_review_is_readable_without_resetting_recovery_accounting() {
    let mut current = seeded();
    let summary =
        json!({"workspace_id":"owned","revision":5,"files":[{"path":"a","sha256":"b".repeat(64)}]});
    let mut edit = receipt("changed", ReceiptState::Completed);
    edit.performed_execution = false;
    edit.result = summary.clone();
    edit.checkpoint = Some(DurableReference {
        reference: "store:changed".into(),
        summary,
    });
    let body = json!({"workspace_id":"owned","revision":5,"file":{"path":"a","sha256":"b".repeat(64)},"offset":0,"next_offset":null,"content":"new source","_durable_read_body":true});
    let mut read = receipt("new-read", ReceiptState::Completed);
    read.performed_execution = false;
    read.result = body.clone();
    read.checkpoint = Some(DurableReference {
        reference: "store:new-read".into(),
        summary: body,
    });
    let mut driver = Fixture::new(
        vec![
            completion(vec![reading("a")]),
            completion(vec![proposal("edit")]),
            completion(vec![reading("a")]),
            completion(vec![]),
        ],
        vec![edit, read],
    );
    let mut limits = policy();
    limits.requires_execution = false;
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
    assert_eq!(driver.calls.len(), 2);
    assert_eq!(driver.notices.len(), 1);
    assert_eq!(current.read_loop_reviews.len(), 1);
    assert_eq!(current.recovery_tokens, 75);
}

#[tokio::test]
async fn failed_process_may_have_changed_source_and_requires_a_fresh_owned_observation() {
    let mut current = seeded();
    current.entries.push(JournalEntry::ToolExchange {invocation_id:"failed-process".into(),tool:"run".into(),arguments:json!({}),result:json!({"_host_sandbox_failure":1,"workspace_id":"owned","revision":5,"files":[{"path":"a","sha256":"b".repeat(64)}]})});
    let mut driver = Fixture::new(
        vec![completion(vec![reading("a")]), completion(vec![])],
        vec![receipt("observed", ReceiptState::Completed)],
    );
    assert!(matches!(
        run(
            &mut driver,
            &mut current,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(driver.calls.len(), 1);
    assert!(driver.notices.is_empty());
}
#[tokio::test]
async fn failed_native_check_guidance_preserves_contracts_and_stops_after_pass_or_new_turn() {
    for variant in ["failed", "passed", "new-user", "external"] {
        let mut current = state();
        let mut summary = json!({"workspace_id":"owned","revision":4,"check":{"status":if variant=="passed" {"passed"} else {"failed"}},"_host_execution":false});
        if variant == "external" {
            summary.as_object_mut().unwrap().remove("_host_execution");
        }
        current.entries.push(JournalEntry::ToolExchange {
            invocation_id: "check".into(),
            tool: "check".into(),
            arguments: json!({}),
            result: summary.clone(),
        });
        current.entries.push(JournalEntry::Checkpoint {
            invocation_id: "check".into(),
            reference: "store:check".into(),
            summary,
        });
        if variant == "new-user" {
            current.entries.push(JournalEntry::User("New task".into()));
        }
        let mut limits = policy();
        limits.requires_execution = false;
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
        let request = serde_json::to_string(&driver.requests[0]).unwrap();
        assert_eq!(request.contains("Host check review"), variant == "failed");
        if variant == "failed" {
            assert!(request.contains("immutable Skill contracts"));
        }
    }
}

#[tokio::test]
async fn shrinking_limits_at_the_same_query_anchor_are_reviewed_with_a_nonpersistent_exact_excerpt()
{
    use sha2::{Digest, Sha256};
    let text = "HEAD\nneedle exact code and more text after it";
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut current = state();
    let summary = json!({"workspace_id":"owned","revision":4,"_host_execution":false,"files":[{"path":"a","sha256":hash}]});
    current.entries.push(JournalEntry::ToolExchange {
        invocation_id: "write-source".into(),
        tool: "write".into(),
        arguments: json!({"files":{"a":text}}),
        result: summary.clone(),
    });
    current.entries.push(JournalEntry::Checkpoint {
        invocation_id: "write-source".into(),
        reference: "store:write-source".into(),
        summary,
    });
    saved_read(
        &mut current,
        "wide",
        "a",
        &hash,
        4,
        5,
        &text[5..],
        serde_json::Value::Null,
        true,
    );
    saved_read(
        &mut current,
        "narrower",
        "a",
        &hash,
        4,
        5,
        &text[5..20],
        json!(20),
        true,
    );
    let mut call = reading("a");
    call.arguments["offset"] = json!(0);
    call.arguments["query"] = json!("needle");
    call.arguments["limit"] = json!(6);
    let mut limits = policy();
    limits.requires_execution = false;
    let mut driver = Fixture::new(vec![completion(vec![call]), completion(vec![])], vec![]);
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
    assert!(driver.calls.is_empty());
    assert_eq!(driver.notices.len(), 1);
    assert_eq!(current.read_loop_reviews[0]["offset"], 5);
    let request = serde_json::to_string(&driver.requests[1]).unwrap();
    assert!(request.contains("requested_excerpt"));
    assert!(request.contains("needle"));
    let cursor = serde_json::to_value(&current).unwrap();
    assert!(
        !cursor["read_loop_reviews"]
            .to_string()
            .contains("requested_excerpt")
    );
}
