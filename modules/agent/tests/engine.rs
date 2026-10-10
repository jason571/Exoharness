use async_trait::async_trait;
use exoharness_agent::{
    Driver, DurableReference, EnginePolicy, EngineState, PendingCall, Receipt, ReceiptState,
    RunOutcome, StopReason, run,
};
use exoharness_contracts::model::{
    ChatRequest, Completion, Finish, Message, Tool, ToolCall, Usage,
};
use exoharness_core::context::JournalEntry;
use serde_json::json;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};
struct Fixture {
    completions: VecDeque<Completion>,
    receipts: VecDeque<Receipt>,
    requests: Vec<ChatRequest>,
    calls: Vec<String>,
    saved: Arc<Mutex<Option<EngineState>>>,
    fail_save: bool,
    cancelled: bool,
}
impl Fixture {
    fn new(completions: Vec<Completion>, receipts: Vec<Receipt>) -> Self {
        Self {
            completions: completions.into(),
            receipts: receipts.into(),
            requests: vec![],
            calls: vec![],
            saved: Arc::new(Mutex::new(None)),
            fail_save: false,
            cancelled: false,
        }
    }
}
#[async_trait]
impl Driver for Fixture {
    fn cancelled(&self) -> bool {
        self.cancelled
    }
    async fn model(&mut self, _: &str, request: ChatRequest) -> Result<Completion, StopReason> {
        self.requests.push(request);
        self.completions
            .pop_front()
            .ok_or(StopReason::ModelIncomplete)
    }
    async fn invoke(&mut self, key: &str, _: &ToolCall) -> Result<Receipt, StopReason> {
        self.calls.push(key.into());
        self.receipts.pop_front().ok_or(StopReason::OutcomeUnknown)
    }
    async fn save(&mut self, state: &EngineState) -> Result<(), StopReason> {
        if self.fail_save {
            return Err(StopReason::PersistenceFailed);
        }
        *self.saved.lock().unwrap() = Some(state.clone());
        Ok(())
    }
}
fn policy() -> EnginePolicy {
    EnginePolicy {
        max_model_calls: 10,
        max_tool_calls: 10,
        max_reported_tokens: 1000,
        max_context_bytes: 100_000,
        output_token_limit: 500,
        requires_execution: true,
    }
}
fn tools() -> Vec<Tool> {
    vec![Tool {
        name: "registered-tool".into(),
        description: "Fixture".into(),
        input_schema: json!({"type":"object"}),
    }]
}
fn state() -> EngineState {
    EngineState::new(vec![JournalEntry::User("do it".into())], 3).unwrap()
}
fn completion(calls: Vec<ToolCall>) -> Completion {
    Completion {
        response_id: None,
        model_name: "test".into(),
        provider_name: "test".into(),
        model_id: "test".into(),
        configuration_revision: 1,
        text: if calls.is_empty() {
            "done".into()
        } else {
            String::new()
        },
        finish: if calls.is_empty() {
            Finish::Stop
        } else {
            Finish::ToolCalls
        },
        calls,
        usage: Usage {
            input_tokens: Some(20),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        },
    }
}
fn proposal(id: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: "registered-tool".into(),
        arguments: json!({"native_array":[{"n":1}]}),
    }
}
fn receipt(id: &str, status: ReceiptState) -> Receipt {
    Receipt {
        invocation_id: id.into(),
        state: status,
        result: json!({"isError":false,"data":"x".repeat(5000)}),
        performed_execution: true,
        checkpoint: None,
    }
}

fn input_rejection(id: &str) -> Receipt {
    let mut r = receipt(id, ReceiptState::RejectedBeforeExecution);
    r.performed_execution = false;
    r.result = json!({"isError":true,"_host_input_rejection":1,"code_executed":false,"diagnostics":{"errors":[{"instance_path":"/files","keyword":"maxProperties","expected":0}]}});
    r
}

#[tokio::test]
async fn schema_correction_preserves_exact_inputs_discards_unsubmitted_dependents_and_executes_once()
 {
    let mut bad = proposal("bad");
    bad.arguments = json!({"source":"EXACT AUTHORED SOURCE","wrong_field":true});
    let mut driver = Fixture::new(
        vec![
            completion(vec![bad, proposal("unsubmitted")]),
            completion(vec![proposal("corrected")]),
            completion(vec![]),
        ],
        vec![
            input_rejection("rejected"),
            receipt("executed", ReceiptState::Completed),
        ],
    );
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
    assert_eq!(driver.calls, vec!["model-1-tool-0", "model-2-tool-0"]);
    assert_eq!(state.successful_executions, 1);
    assert_eq!(state.input_corrections, 1);
    let correction_request = serde_json::to_string(&driver.requests[1]).unwrap();
    assert!(correction_request.contains("EXACT AUTHORED SOURCE"));
    assert!(correction_request.contains("maxProperties"));
}

#[tokio::test]
async fn changing_invalid_arguments_cannot_bypass_correction_budget_or_resume() {
    let completions = (0..3)
        .map(|n| {
            let mut c = proposal(&format!("bad-{n}"));
            c.arguments = json!({"wrong":n});
            completion(vec![c])
        })
        .collect();
    let receipts = (0..3)
        .map(|n| input_rejection(&format!("reject-{n}")))
        .collect();
    let mut driver = Fixture::new(completions, receipts);
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
        RunOutcome::Stopped(StopReason::InputRejected)
    );
    assert_eq!(driver.requests.len(), 3);
    assert_eq!(state.input_corrections, 2);
    assert_eq!(state.successful_executions, 0);
    let serialized = serde_json::to_string(&state).unwrap();
    let mut recovered: EngineState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        run(
            &mut driver,
            &mut recovered,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::InputRejected)
    );
    assert_eq!(driver.requests.len(), 3);
}

#[tokio::test]
async fn permission_unknown_and_after_execution_errors_never_enter_input_correction() {
    for status in [
        ReceiptState::RejectedBeforeExecution,
        ReceiptState::FailedAfterExecution,
        ReceiptState::OutcomeUnknown,
    ] {
        let mut r = input_rejection("failure");
        r.state = status;
        if status == ReceiptState::RejectedBeforeExecution {
            r.result = json!({"isError":true,"permission":"denied"});
        }
        let mut driver = Fixture::new(vec![completion(vec![proposal("p")])], vec![r]);
        let mut state = state();
        assert!(matches!(
            run(
                &mut driver,
                &mut state,
                &policy(),
                "system",
                &BTreeSet::new(),
                &tools()
            )
            .await,
            RunOutcome::Stopped(_)
        ));
        assert_eq!(driver.requests.len(), 1);
        assert_eq!(state.input_corrections, 0);
    }
}

#[tokio::test]
async fn persisted_required_workflow_cannot_be_finished_by_text_or_cleared_by_reload() {
    let mut driver = Fixture::new(vec![completion(vec![])], vec![]);
    let mut state = state();
    state.successful_executions = 1;
    state.entries.extend([
        JournalEntry::ToolExchange { invocation_id:"saved".into(),tool:"registered-tool".into(),arguments:json!({}),result:json!({}) },
        JournalEntry::Checkpoint { invocation_id:"saved".into(),reference:"urn:saved".into(),summary:json!({"isError":false,"_host_workflow":{"version":1,"capability":"provider","directive":{"status":"required","context_scope":"checkpoint"},"tools":["registered-tool"],"provider_tools":["registered-tool"]}}) },
        JournalEntry::ToolExchange { invocation_id:"reload".into(),tool:"activate_tools".into(),arguments:json!({"names":["registered-tool"]}),result:json!({}) },
    ]);
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
        RunOutcome::Stopped(StopReason::MissingExecution)
    );
    assert!(state.answer.is_none());
    assert!(driver.calls.is_empty());
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
        RunOutcome::Stopped(StopReason::MissingExecution)
    );
    assert_eq!(driver.requests.len(), 1);
}

#[tokio::test]
async fn model_tool_model_is_native_paired_and_final_state_never_calls_again() {
    let mut driver = Fixture::new(
        vec![
            completion(vec![proposal("provider-id")]),
            completion(vec![]),
        ],
        vec![receipt("host-id", ReceiptState::Completed)],
    );
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
    assert_eq!(driver.calls, vec!["model-1-tool-0"]);
    let second = &driver.requests[1];
    assert!(
        matches!(&second.messages[2], Message::Assistant{calls,..} if calls[0].id=="host-id" && calls[0].arguments==proposal("unused").arguments)
    );
    assert!(matches!(&second.messages[3], Message::ToolResult{call_id,..} if call_id=="host-id"));
    assert_eq!(state.reported_tokens, 50);
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
    assert_eq!(driver.requests.len(), 2);
}
#[tokio::test]
async fn durable_receipt_replaces_large_exchange_before_the_next_model() {
    let mut result = receipt("host-id", ReceiptState::Completed);
    result.checkpoint = Some(DurableReference {
        reference: "store://one".into(),
        summary: json!({"next":"deliver"}),
    });
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("p")]), completion(vec![])],
        vec![result],
    );
    let mut state = state();
    assert!(matches!(
        run(
            &mut driver,
            &mut state,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    let request = serde_json::to_string(&driver.requests[1]).unwrap();
    assert!(request.contains("store://one"));
    assert!(!request.contains(&"x".repeat(1000)));
    assert!(
        matches!(&state.entries[1],JournalEntry::ToolExchange{result,..} if result["data"].as_str().unwrap().len()==5000)
    );
}
#[tokio::test]
async fn unknown_outcome_and_unchanged_recovery_never_reinvoke_or_ask_the_model() {
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("p")])],
        vec![receipt("host-id", ReceiptState::OutcomeUnknown)],
    );
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
        RunOutcome::Stopped(StopReason::OutcomeUnknown)
    );
    let encoded = serde_json::to_string(&state).unwrap();
    let mut recovered: EngineState = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        run(
            &mut driver,
            &mut recovered,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::OutcomeUnknown)
    );
    assert_eq!(driver.calls.len(), 1);
    assert_eq!(driver.requests.len(), 1);
}
#[tokio::test]
async fn pending_receipt_recovery_uses_saved_key_without_repeating_budget_admission() {
    let mut state = state();
    state.model_calls = 1;
    state.tool_calls = 1;
    state.pending = vec![PendingCall {
        key: "model-1-tool-0".into(),
        call: proposal("p"),
        admitted: true,
    }];
    let mut driver = Fixture::new(
        vec![completion(vec![])],
        vec![receipt("host-id", ReceiptState::Completed)],
    );
    assert!(matches!(
        run(
            &mut driver,
            &mut state,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(state.tool_calls, 1);
    assert_eq!(driver.calls, vec!["model-1-tool-0"]);
}
#[tokio::test]
async fn identical_consecutive_calls_stop_at_three_and_recovery_does_not_reset_guard() {
    let mut driver = Fixture::new(
        (0..4)
            .map(|i| completion(vec![proposal(&format!("p{i}"))]))
            .collect(),
        (0..3)
            .map(|i| receipt(&format!("host-{i}"), ReceiptState::Completed))
            .collect(),
    );
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
        RunOutcome::Stopped(StopReason::RepeatedCall)
    );
    assert_eq!(driver.calls.len(), 3);
    let mut restored: EngineState =
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
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
        RunOutcome::Stopped(StopReason::RepeatedCall)
    );
    assert_eq!(driver.requests.len(), 4);
}
#[tokio::test]
async fn failed_durable_write_and_uncertain_model_stop_before_new_side_effects() {
    let mut driver = Fixture::new(vec![], vec![]);
    driver.fail_save = true;
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
        RunOutcome::Stopped(StopReason::PersistenceFailed)
    );
    driver.fail_save = false;
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
        RunOutcome::Stopped(StopReason::PersistenceFailed)
    );
    assert!(driver.requests.is_empty());
    let mut uncertain = super_state();
    uncertain.in_flight_model = Some("model-1".into());
    assert_eq!(
        run(
            &mut driver,
            &mut uncertain,
            &policy(),
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::ModelIncomplete)
    );
    assert!(driver.requests.is_empty());
}
fn super_state() -> EngineState {
    state()
}

#[tokio::test]
async fn token_budget_stops_before_dispatch_and_read_receipts_cannot_fake_execution() {
    let mut driver = Fixture::new(vec![completion(vec![proposal("p")])], vec![]);
    let mut state = super_state();
    let mut limit = policy();
    limit.max_reported_tokens = 25;
    assert_eq!(
        run(
            &mut driver,
            &mut state,
            &limit,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::TokenBudget)
    );
    assert!(driver.calls.is_empty());
    let mut read = receipt("read-id", ReceiptState::Completed);
    read.performed_execution = false;
    let mut driver = Fixture::new(
        vec![completion(vec![proposal("p")]), completion(vec![])],
        vec![read],
    );
    let mut state = super_state();
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
        RunOutcome::Stopped(StopReason::MissingExecution)
    );
}

#[tokio::test]
async fn changing_native_arguments_resets_the_consecutive_guard() {
    let mut proposals = vec![];
    for i in 0..4 {
        let mut call = proposal(&format!("p{i}"));
        call.arguments = json!({"n":i});
        proposals.push(completion(vec![call]));
    }
    proposals.push(completion(vec![]));
    let mut driver = Fixture::new(
        proposals,
        (0..4)
            .map(|i| receipt(&format!("host{i}"), ReceiptState::Completed))
            .collect(),
    );
    let mut state = super_state();
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
    assert_eq!(driver.calls.len(), 4);
}
#[tokio::test]
async fn invalid_proposal_missing_execution_cancellation_and_budgets_fail_closed() {
    let mut wrong = proposal("p");
    wrong.arguments = json!("encoded JSON is not native");
    let mut driver = Fixture::new(vec![completion(vec![wrong])], vec![]);
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
        RunOutcome::Stopped(StopReason::InvalidProposal)
    );
    assert!(driver.calls.is_empty());
    let mut driver = Fixture::new(vec![completion(vec![])], vec![]);
    let mut state = super_state();
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
        RunOutcome::Stopped(StopReason::MissingExecution)
    );
    let mut driver = Fixture::new(vec![], vec![]);
    driver.cancelled = true;
    let mut state = super_state();
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
        RunOutcome::Stopped(StopReason::Cancelled)
    );
    assert!(driver.requests.is_empty());
    let mut driver = Fixture::new(vec![], vec![]);
    let mut state = super_state();
    let mut policy = policy();
    policy.max_context_bytes = 1;
    assert_eq!(
        run(
            &mut driver,
            &mut state,
            &policy,
            "system",
            &BTreeSet::new(),
            &tools()
        )
        .await,
        RunOutcome::Stopped(StopReason::ContextBudget)
    );
    assert!(driver.requests.is_empty());
}
