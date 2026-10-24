#[path = "../../benchmark/tests/support/mod.rs"]
mod support;
use exoharness_benchmark::{Harness, Management, Suite, digest, evidence};
use exoharness_evolution::{
    contracts::{Policy, candidate},
    controller::Controller,
    ledger::consume_holdout,
    package,
    publisher::Publisher,
    trajectories::Trajectories,
};
use serde_json::{Value, json};
fn suite(review: bool) -> Suite {
    Suite::parse(json!({"version":1,"id":"evolution_fixture","tasks":[
    {"id":"train_task","group":"pricing","split":"train","turns":["Compute training amount"],"checks":[{"kind":"json_equals","fields":{"amount":42}}]},
    {"id":"validation_task","group":"pricing","split":"validation","turns":["Compute validation amount"],"checks":[{"kind":"json_equals","fields":{"amount":42}}],"review_required":review},
    {"id":"test_task","group":"pricing","split":"test","turns":["Compute test amount"],"checks":[{"kind":"json_equals","fields":{"amount":42}}],"review_required":review}
]})).unwrap()
}
fn experience(suite: &Suite) -> Value {
    json!({"version":1,"suite_sha256":suite.sha256(),"rules":[{"group":"pricing","sources":["train_task"],"when":"A pricing task is requested.","action":"Apply the confirmed multiplier to the amount.","unless":"Required values are missing."}]})
}
fn policy(suite: &Suite, hash: &str, publish: bool) -> Policy {
    Policy::parse(json!({"version":1,"id":"pricing_v1","group":"pricing","skill_name":"experience-pricing","description":"Reviewed business pricing procedures","scope":{"kind":"global"},"baseline_binary_sha256":hash,"suite_sha256":suite.sha256(),"run_reservation":1000,"max_tokens":100000,"supervision":"fewshot","publish":publish})).unwrap()
}
#[test]
fn policy_limits_and_authority_are_validated() {
    let s = suite(false);
    let p = policy(&s, &"a".repeat(64), false);
    for (k, value) in [
        ("publish", json!(1)),
        ("repeats", json!(2)),
        ("skill_name", json!("ordinary-skill")),
        ("max_tokens", json!(5)),
        ("scope", json!({"kind":"global","user_id":"other"})),
        ("method", json!("unknown")),
        ("auto_suspend", json!(true)),
    ] {
        let mut v = p.value().clone();
        v[k] = value;
        assert!(Policy::parse(v).is_err(), "{k}");
    }
    assert_eq!(
        Policy::parse(p.value().clone()).unwrap().sha256(),
        p.sha256()
    );
}
#[test]
fn packages_keep_managed_identity_and_never_transport_authority() {
    let s = suite(false);
    let p = policy(&s, &"a".repeat(64), false);
    let body = package::compile(
        &p,
        &s,
        &experience(&s),
        "round-0",
        &[],
        &Trajectories::default(),
    )
    .unwrap();
    package::validate_inventory(&body).unwrap();
    let mut changed = body.clone();
    changed["files"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path":"code.py","content_base64":"cHJpbnQoMSk="}));
    assert!(package::validate_inventory(&changed).is_err());
    let mut leaked = experience(&s);
    leaked["rules"][0]["action"] = json!("Use Bearer PRIVATE_TOKEN");
    assert!(package::compile(&p, &s, &leaked, "round-0", &[], &Trajectories::default()).is_err());
    assert_ne!(
        package::package_digest(&body).unwrap(),
        package::package_digest(&changed).unwrap()
    );
}
#[test]
fn holdout_cannot_be_reused_for_another_candidate() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("holdouts");
    let s = suite(false);
    consume_holdout(&root, &s, "pricing", "first").unwrap();
    consume_holdout(&root, &s, "pricing", "first").unwrap();
    assert!(consume_holdout(&root, &s, "pricing", "second").is_err());
}
#[test]
fn complex_workflow_keeps_more_than_eight_observed_steps() {
    let s = suite(false);
    let mut pv = policy(&s, &"a".repeat(64), false).value().clone();
    pv["method"] = json!("workflow");
    pv["max_rule_characters"] = json!(4800);
    let p = Policy::parse(pv).unwrap();
    let steps: Vec<_> = (0..12)
        .map(|i| json!({"tool":format!("step_{i}"),"action":"Do the step.","expect":"A receipt."}))
        .collect();
    let trajectory = Trajectories {
        rows: vec![
            json!({"train_id":"train_task","verified_delivery":true,"steps":steps.iter().map(|s|json!({"tool":s["tool"],"outcome":"success"})).collect::<Vec<_>>()}),
        ],
        identity: Some("b".repeat(64)),
    };
    let v = json!({"version":2,"suite_sha256":s.sha256(),"rules":[],"workflows":[{"id":"pricing-workflow","group":"pricing","sources":["train_task"],"when":"A price is needed.","unless":"Inputs are missing.","terms":["pricing"],"steps":steps}]});
    assert!(candidate(v.clone(), &p, &s, &trajectory).is_ok());
    let body = package::compile(&p, &s, &v, "round-0", &[], &trajectory).unwrap();
    assert!(
        package::files(&body)
            .unwrap()
            .contains_key("resource-index.json")
    );
    let mut wrong = v;
    wrong["workflows"][0]["steps"][0]["tool"] = json!("unobserved");
    assert!(candidate(wrong, &p, &s, &trajectory).is_err());
}
#[tokio::test]
async fn lost_import_reply_reconciles_exact_digest_without_reimport() {
    let s = suite(false);
    let f = support::Fixture::start(1, experience(&s)).await;
    f.state.lock().unwrap().lost_import = true;
    let temp = tempfile::tempdir().unwrap();
    let auth = temp.path().join("auth.json");
    let a = f.auth(&auth);
    let p = policy(&s, a["runtime_version"].as_str().unwrap(), false);
    let client = Management::new(&auth).unwrap();
    let publisher = Publisher::new(&client, &p);
    let body = package::compile(
        &p,
        &s,
        &experience(&s),
        "round-0",
        &[],
        &Trajectories::default(),
    )
    .unwrap();
    let root = temp.path().join("publication");
    assert!(publisher.publish(&body, &root, &Value::Null).await.is_err());
    let receipt = publisher.publish(&body, &root, &Value::Null).await.unwrap();
    assert_eq!(receipt["phase"], "ready");
    assert_eq!(f.state.lock().unwrap().imports, 1);
    publisher.publish(&body, &root, &Value::Null).await.unwrap();
    assert_eq!(f.state.lock().unwrap().imports, 1);
}
#[tokio::test]
async fn full_lifecycle_publishes_only_measured_improvement_and_resumes() {
    let s = suite(false);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let baseline = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let controller =
        Controller::new(&p, &s, &baseline, &canary, &management, &root, None, None).unwrap();
    let state = controller.run().await.unwrap();
    assert_eq!(state["phase"], "published");
    assert_eq!(prod.state.lock().unwrap().imports, 1);
    assert_eq!(
        state["charges"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        617
    );
    let before = a.state.lock().unwrap().turns + b.state.lock().unwrap().turns;
    assert_eq!(controller.run().await.unwrap()["phase"], "published");
    assert_eq!(
        a.state.lock().unwrap().turns + b.state.lock().unwrap().turns,
        before
    );
    assert_eq!(
        controller.monitor().await.unwrap()["status"],
        "not_configured"
    );
}
#[tokio::test]
async fn regression_is_rejected_before_heldout_or_production_publication() {
    let s = suite(false);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    {
        let mut state = b.state.lock().unwrap();
        state.amount = 41;
        state.training_amounts = vec![42];
    }
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let baseline = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let controller =
        Controller::new(&p, &s, &baseline, &canary, &management, &root, None, None).unwrap();
    let state = controller.run().await.unwrap();
    assert_eq!(state["phase"], "validation_rejected");
    assert_eq!(prod.state.lock().unwrap().imports, 0);
    assert!(!root.join("round-0-test").exists());
    assert!(controller.publish().await.is_err());
}

#[tokio::test]
async fn fewshot_training_failure_stops_before_validation_and_preserves_terminal_budget() {
    let s = suite(false);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    b.state.lock().unwrap().amount = 41;
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let mut pv = policy(&s, auth["runtime_version"].as_str().unwrap(), true)
        .value()
        .clone();
    pv["max_rounds"] = json!(1);
    let p = Policy::parse(pv).unwrap();
    let baseline = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let c = Controller::new(&p, &s, &baseline, &canary, &management, &root, None, None).unwrap();
    let state = c.run().await.unwrap();
    assert_eq!(state["phase"], "exhausted");
    assert_eq!(state["rounds"][0]["reason"], "train_feedback");
    assert_eq!(exoharness_benchmark::campaign::charges(&state).unwrap(), 11);
    let feedback = evidence::read(&root.join("round-0/train-feedback.json")).unwrap();
    assert_eq!(feedback[0]["passed"], false);
    assert_eq!(feedback[0]["answer"], "{\"amount\":41,\"product\":312}");
    assert!(!root.join("round-0-validation").exists());
    assert!(!root.join("round-0-test").exists());
    assert_eq!(prod.state.lock().unwrap().imports, 0);
    assert_eq!(a.state.lock().unwrap().turns, 1);
    assert_eq!(b.state.lock().unwrap().turns, 1);
    assert_eq!(c.run().await.unwrap(), state);
    assert_eq!(a.state.lock().unwrap().turns, 1);
    assert_eq!(b.state.lock().unwrap().turns, 1);
}

#[tokio::test]
async fn fewshot_repairs_from_training_only_and_accounts_for_every_attempt() {
    let s = suite(false);
    let original = experience(&s);
    let mut corrected = original.clone();
    corrected["rules"][0]["action"] = json!(
        "Resolve the current input values, apply the confirmed multiplier and verify the result against the stated constraints."
    );
    let a = support::Fixture::start(100, corrected.clone()).await;
    a.state.lock().unwrap().learner_candidates = vec![original, corrected];
    let b = support::Fixture::start(1, experience(&s)).await;
    b.state.lock().unwrap().training_amounts = vec![41, 42];
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let baseline = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let c = Controller::new(&p, &s, &baseline, &canary, &management, &root, None, None).unwrap();
    let state = c.run().await.unwrap();
    assert_eq!(state["phase"], "published");
    assert_eq!(state["rounds"].as_array().unwrap().len(), 2);
    assert_eq!(state["rounds"][0]["reason"], "train_feedback");
    assert_eq!(
        exoharness_benchmark::campaign::charges(&state).unwrap(),
        628
    );
    assert!(!root.join("round-0-validation").exists());
    assert!(root.join("round-1-test").exists());
    let calls = &a.state.lock().unwrap().learner_prompts;
    assert_eq!(calls.len(), 2);
    assert!(calls[1].contains("\\\"amount\\\":41"));
    for prompt in calls {
        assert!(prompt.contains("Compute training amount"));
        assert!(!prompt.contains("Compute validation amount"));
        assert!(!prompt.contains("Compute test amount"));
    }
    assert_eq!(prod.state.lock().unwrap().imports, 1);
}

#[tokio::test]
async fn review_resume_is_bound_to_saved_evidence_and_never_reexecutes_validation() {
    let s = suite(true);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let base = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let c = Controller::new(&p, &s, &base, &canary, &management, &root, None, None).unwrap();
    let state = c.run().await.unwrap();
    assert_eq!(state["rounds"][0]["phase"], "awaiting_validation_review");
    assert_eq!(prod.state.lock().unwrap().imports, 0);
    for split in ["validation", "test"] {
        let folder = root.join(format!("round-0-{split}"));
        let report = evidence::read(&folder.join("evolution-report.json")).unwrap();
        let decisions: Vec<_> = report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| json!({"key":r["key"],"passed":true}))
            .collect();
        evidence::write(
            &folder.join("delivery-review.json"),
            &json!({"report_sha256":digest(&report),"reviewer":"qa","decisions":decisions}),
        )
        .unwrap();
        let state = c.run().await.unwrap();
        assert_eq!(
            state["rounds"][0]["phase"],
            if split == "validation" {
                "awaiting_test_review"
            } else {
                "validated"
            }
        );
    }
    assert_eq!(prod.state.lock().unwrap().imports, 1);
    assert_eq!(a.state.lock().unwrap().turns, 7);
    assert_eq!(b.state.lock().unwrap().turns, 7);
}

#[tokio::test]
async fn learner_execution_unknown_usage_and_overrun_stop_without_retries() {
    for mode in ["tool", "unknown", "overrun"] {
        let s = suite(false);
        let a = support::Fixture::start(if mode == "overrun" { 2000 } else { 100 }, experience(&s))
            .await;
        {
            let mut state = a.state.lock().unwrap();
            state.learner_uses_tool = mode == "tool";
            state.unknown_usage = mode == "unknown";
        }
        let b = support::Fixture::start(1, experience(&s)).await;
        let prod = support::Fixture::start(100, experience(&s)).await;
        let temp = tempfile::tempdir().unwrap();
        let ap = temp.path().join("a.json");
        let bp = temp.path().join("b.json");
        let pp = temp.path().join("p.json");
        let auth = a.auth(&ap);
        b.auth(&bp);
        prod.auth(&pp);
        let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
        let base = Harness::new(&ap, 1000, 30).unwrap();
        let canary = Harness::new(&bp, 1000, 30).unwrap();
        let management = Management::new(&pp).unwrap();
        let root = temp.path().join("campaign");
        let c = Controller::new(&p, &s, &base, &canary, &management, &root, None, None).unwrap();
        assert!(c.run().await.is_err(), "{mode}");
        let before = a.state.lock().unwrap().turns;
        assert!(c.run().await.is_err());
        assert_eq!(a.state.lock().unwrap().turns, before);
        assert_eq!(prod.state.lock().unwrap().imports, 0);
        let ledger = evidence::read(&root.join("evolution.json")).unwrap();
        assert!(
            exoharness_benchmark::campaign::charges(&ledger).unwrap()
                >= if mode == "unknown" {
                    1000
                } else if mode == "overrun" {
                    2010
                } else {
                    10
                }
        );
    }
}

#[tokio::test]
async fn incorrect_skill_read_revision_cannot_pass_the_gate() {
    let s = suite(false);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    b.state.lock().unwrap().wrong_read_revision = true;
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let base = Harness::new(&ap, 1000, 30).unwrap();
    let canary = Harness::new(&bp, 1000, 30).unwrap();
    let management = Management::new(&pp).unwrap();
    let root = temp.path().join("campaign");
    let c = Controller::new(&p, &s, &base, &canary, &management, &root, None, None).unwrap();
    assert_eq!(c.run().await.unwrap()["phase"], "unchanged");
    let report = evidence::read(&root.join("round-0/train-feedback.json")).unwrap();
    assert!(
        report[0]["checks"].as_array().unwrap().last().unwrap()["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("experience_not_read_in_every_candidate_attempt"))
    );
    assert!(!root.join("round-0-validation").exists());
    assert_eq!(prod.state.lock().unwrap().imports, 0);
}

#[tokio::test]
async fn monitor_uses_exact_revision_excludes_cancellation_and_only_suspends_its_package() {
    for tamper in [false, true] {
        let s = suite(false);
        let f = support::Fixture::start(1, experience(&s)).await;
        let temp = tempfile::tempdir().unwrap();
        let authpath = temp.path().join("auth.json");
        let auth = f.auth(&authpath);
        let mut pv = policy(&s, auth["runtime_version"].as_str().unwrap(), false)
            .value()
            .clone();
        pv["monitor_user_ids"] = json!(["approved_owner"]);
        pv["monitor_min_samples"] = json!(2);
        pv["auto_suspend"] = json!(true);
        let p = Policy::parse(pv).unwrap();
        let management = Management::new(&authpath).unwrap();
        let publisher = Publisher::new(&management, &p);
        let body = package::compile(
            &p,
            &s,
            &experience(&s),
            "round-0",
            &[],
            &Trajectories::default(),
        )
        .unwrap();
        let receipt = publisher
            .publish(&body, &temp.path().join("publication"), &Value::Null)
            .await
            .unwrap();
        let root = temp.path().join("monitor");
        let mut ledger =
            exoharness_evolution::ledger::Ledger::open(&root, json!({"fixture":true})).unwrap();
        ledger.state["publications"] = json!([receipt]);
        ledger.save().unwrap();
        {
            let mut state = f.state.lock().unwrap();
            state.trace_sessions =
                vec![json!({"id":"session-owned","owner_user_id":"approved_owner"})];
            for (id, status, revision) in [
                ("r1", "failed", 1),
                ("r2", "succeeded", 1),
                ("r3", "cancelled", 1),
                ("r4", "failed", 99),
            ] {
                state
                    .trace_runs
                    .push(json!({"id":id,"status":status,"created_at":1}));
                state.trace_records.insert(id.into(),json!({"run":{"id":id,"status":status},"model_calls":[{"usage_reported":true,"usage":{"input_tokens":1,"output_tokens":1}}],"invocations":[{"tool_name":"read_skill","arguments":{"id":p.skill_id()},"result":{"revision":revision}}]}));
            }
            if tamper {
                state.record["source_digest"] = json!("b".repeat(64));
            }
        }
        let result = exoharness_evolution::monitor::observe(&publisher, &mut ledger).await;
        if tamper {
            assert!(result.is_err());
            assert_eq!(f.state.lock().unwrap().record["enabled"], true);
        } else {
            let report = result.unwrap();
            assert_eq!(report["samples"], 2);
            assert_eq!(report["failures"], 1);
            assert_eq!(report["suspended"], true);
            assert_eq!(f.state.lock().unwrap().record["enabled"], false);
        }
    }
}

#[tokio::test]
async fn published_queue_does_not_restart_evaluation_and_budget_identity_is_immutable() {
    let s = suite(false);
    let a = support::Fixture::start(100, experience(&s)).await;
    let b = support::Fixture::start(1, experience(&s)).await;
    let prod = support::Fixture::start(100, experience(&s)).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    let pp = temp.path().join("p.json");
    let auth = a.auth(&ap);
    b.auth(&bp);
    prod.auth(&pp);
    let p = policy(&s, auth["runtime_version"].as_str().unwrap(), true);
    let policyfile = temp.path().join("policy.json");
    let suitefile = temp.path().join("suite.json");
    evidence::write(&policyfile, p.value()).unwrap();
    evidence::write(&suitefile, s.value()).unwrap();
    let queue = temp.path().join("queue");
    let root = temp.path().join("queue-state");
    evidence::write(&queue.join("job.json"),&json!({"version":1,"id":"native_campaign","policy":policyfile,"suite":suitefile,"baseline_auth":ap,"canary_auth":bp,"publication_auth":pp,"input_root":temp.path()})).unwrap();
    let first = exoharness_evolution::queue::tick(&queue, &root, 100000)
        .await
        .unwrap();
    assert_eq!(first["jobs"]["native_campaign"]["phase"], "published");
    drop(a);
    drop(b);
    let mut saved = evidence::read(&root.join("queue.json")).unwrap();
    saved["jobs"]["native_campaign"]["last_monitor_at"] = json!(0);
    evidence::write(&root.join("queue.json"), &saved).unwrap();
    let again = exoharness_evolution::queue::tick(&queue, &root, 100000)
        .await
        .unwrap();
    assert_eq!(again["jobs"]["native_campaign"]["phase"], "published");
    assert_eq!(prod.state.lock().unwrap().imports, 1);
    assert!(
        exoharness_evolution::queue::tick(&queue, &root, 99999)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn workflow_publication_requires_matching_resource_receipts_and_exports_immutable_files() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../scripts/tests/fixtures/rust-evaluation/workflow.json"
    ))
    .unwrap();
    let s = Suite::parse(golden["suite"].clone()).unwrap();
    for missing in [true, false] {
        let a = support::Fixture::start(100, golden["candidate"].clone()).await;
        let b = support::Fixture::start(1, golden["candidate"].clone()).await;
        b.state.lock().unwrap().omit_resource_read = missing;
        let prod = support::Fixture::start(100, golden["candidate"].clone()).await;
        let temp = tempfile::tempdir().unwrap();
        let ap = temp.path().join("a.json");
        let bp = temp.path().join("b.json");
        let pp = temp.path().join("p.json");
        let auth = a.auth(&ap);
        b.auth(&bp);
        prod.auth(&pp);
        let mut pv = policy(&s, auth["runtime_version"].as_str().unwrap(), true)
            .value()
            .clone();
        pv["method"] = json!("workflow");
        let p = Policy::parse(pv).unwrap();
        let corpus = temp.path().join("evidence");
        let files = [
            (
                "manifest.json",
                include_str!(
                    "../../../scripts/tests/fixtures/rust-evaluation/training-evidence/manifest.json"
                ),
            ),
            (
                "sample/execution.json",
                include_str!(
                    "../../../scripts/tests/fixtures/rust-evaluation/training-evidence/sample/execution.json"
                ),
            ),
            (
                "sample/outcome.json",
                include_str!(
                    "../../../scripts/tests/fixtures/rust-evaluation/training-evidence/sample/outcome.json"
                ),
            ),
            (
                "sample/session.json",
                include_str!(
                    "../../../scripts/tests/fixtures/rust-evaluation/training-evidence/sample/session.json"
                ),
            ),
            (
                "sample/trace-0.json",
                include_str!(
                    "../../../scripts/tests/fixtures/rust-evaluation/training-evidence/sample/trace-0.json"
                ),
            ),
        ];
        for (file, content) in files {
            evidence::write(&corpus.join(file), &serde_json::from_str(content).unwrap()).unwrap();
        }
        let base = Harness::new(&ap, 1000, 30).unwrap();
        let canary = Harness::new(&bp, 1000, 30).unwrap();
        let management = Management::new(&pp).unwrap();
        let root = temp.path().join("campaign");
        let c = Controller::new(
            &p,
            &s,
            &base,
            &canary,
            &management,
            &root,
            None,
            Some(&corpus),
        )
        .unwrap();
        let state = c.run().await.unwrap();
        assert_eq!(
            state["phase"],
            if missing { "unchanged" } else { "published" }
        );
        assert_eq!(prod.state.lock().unwrap().imports, u64::from(!missing));
        if !missing {
            let snapshot = c.production.snapshot().await.unwrap().unwrap();
            package::validate_inventory(&snapshot).unwrap();
            assert!(
                package::files(&snapshot)
                    .unwrap()
                    .contains_key("resource-index.json")
            );
        }
    }
}
