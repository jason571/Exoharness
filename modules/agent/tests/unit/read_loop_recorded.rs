//! Private recorded regression; no model, sandbox or external capability calls.
use super::*;

#[tokio::test]
#[ignore = "requires private read-loop Trace and request manifests"]
async fn recorded_233524_run_reviews_third_read_without_resetting_consumed_budget() {
    let root = std::path::PathBuf::from(
        std::env::var("EXOHARNESS_READ_LOOP_REPLAY").expect("private evidence directory"),
    );
    let trace: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("trace.json")).unwrap()).unwrap();
    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("18.json")).unwrap()).unwrap();
    let mut available = vec![];
    for part in request["evidence"]["manifest"]["parts"].as_array().unwrap() {
        if part["kind"] == "tool" {
            available.push(
                serde_json::from_value(
                    request["parts"][part["sha256"].as_str().unwrap()]["content"].clone(),
                )
                .unwrap(),
            );
        }
    }
    let calls = trace["model_calls"].as_array().unwrap();
    let mut scripted = vec![];
    for m in &calls[..17] {
        let mut reply =
            completion(serde_json::from_value(m["output"]["proposed_calls"].clone()).unwrap());
        reply.text = m["output"]["answer"].as_str().unwrap().into();
        reply.usage.input_tokens = m["usage"]["input_tokens"].as_u64();
        reply.usage.output_tokens = m["usage"]["output_tokens"].as_u64();
        scripted.push(reply);
    }
    let mut publish = proposal("corrective-publication");
    publish.name = "workspace_publish".into();
    publish.arguments = json!({"workspace_id":trace["invocations"][3]["result"]["workspace_id"],"expected_revision":4,"output_files":["index.html"]});
    scripted.push(completion(vec![publish]));
    scripted.push(completion(vec![]));
    let mut driver = Fixture::new(scripted, vec![]);
    for i in trace["invocations"].as_array().unwrap() {
        let id = i["id"].as_str().unwrap();
        let key = id.split_once(':').unwrap().1.to_owned();
        let value = i["result"].clone();
        let status = if value["_host_input_rejection"] == 1 {
            ReceiptState::RejectedBeforeExecution
        } else {
            ReceiptState::Completed
        };
        let read = value["file"]["sha256"].is_string() && value["content"].is_string();
        let checkpoint = if read || value["_workspace_checkpoint"] == true {
            let mut summary = value.clone();
            summary
                .as_object_mut()
                .unwrap()
                .remove("_workspace_checkpoint");
            if read {
                summary["_durable_read_body"] = json!(true);
            }
            Some(DurableReference {
                reference: format!("store:{id}"),
                summary,
            })
        } else {
            None
        };
        driver.keyed_receipts.insert(
            key,
            Receipt {
                invocation_id: id.into(),
                state: status,
                result: value,
                performed_execution: false,
                checkpoint,
            },
        );
    }
    driver.keyed_receipts.insert(
        "model-18-tool-0".into(),
        Receipt {
            invocation_id: "mock-recorded-publication".into(),
            state: ReceiptState::Completed,
            result: json!({"artifacts":[{"filename":"index.html"}]}),
            performed_execution: false,
            checkpoint: None,
        },
    );
    let mut current = state();
    let mut limits = policy();
    limits.max_model_calls = 24;
    limits.max_tool_calls = 48;
    limits.max_reported_tokens = 300_000;
    limits.max_recovery_tokens = 196_608;
    limits.max_context_bytes = 1024 * 1024;
    limits.requires_execution = false;
    let outcome = run(
        &mut driver,
        &mut current,
        &limits,
        "system",
        &BTreeSet::new(),
        &available,
    )
    .await;
    assert!(
        matches!(
            outcome,
            RunOutcome::Stopped(StopReason::RecoveryTokenBudget)
        ),
        "outcome={outcome:?} models={} dispatched={} notices={} tokens={}",
        driver.requests.len(),
        driver.calls.len(),
        driver.notices.len(),
        current.recovery_tokens
    );
    assert_eq!(driver.notices.len(), 1);
    assert_eq!(driver.notices[0]["review"]["path"], "index.html");
    assert!(!driver.calls.contains(&"model-17-tool-0".into()));
    assert!(!driver.calls.contains(&"model-18-tool-0".into()));
    assert_eq!(current.recovery_tokens, 124331);
    let projected = serde_json::to_value(&driver.requests[16])
        .unwrap()
        .to_string();
    for source in ["index.html", "script.js", "check_site.py"] {
        assert!(projected.contains(source));
    }
    println!(
        "Recorded run: third full read reviewed before dispatch; exhausted conservative allowance still stops new requests without resetting consumed usage. No new model/tool requests; recorded usage is not a new cost measurement."
    );
}
