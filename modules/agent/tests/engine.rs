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
#[path = "unit/failure_finalization.rs"]
mod failure_finalization;
#[path = "unit/model_correction.rs"]
mod model_correction;
#[path = "unit/read_loop.rs"]
mod read_loop;
#[path = "unit/read_loop_recorded.rs"]
mod read_loop_recorded;
#[path = "unit/recovery_budget.rs"]
mod recovery_budget;
#[path = "unit/run_budget.rs"]
mod run_budget;
#[path = "unit/sandbox_episode.rs"]
mod sandbox_episode;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};
struct Fixture {
    model_failures: std::collections::BTreeMap<usize, exoharness_agent::FailedModel>,
    failed_model: Option<exoharness_agent::FailedModel>,
    initial_discovery: Option<exoharness_agent::InitialDiscovery>,
    completions: VecDeque<Completion>,
    receipts: VecDeque<Receipt>,
    keyed_receipts: std::collections::BTreeMap<String, Receipt>,
    requests: Vec<ChatRequest>,
    calls: Vec<String>,
    saved: Arc<Mutex<Option<EngineState>>>,
    fail_save: bool,
    cancelled: bool,
    notices: Vec<serde_json::Value>,
    fail_notice: bool,
}
impl Fixture {
    fn new(completions: Vec<Completion>, receipts: Vec<Receipt>) -> Self {
        Self {
            model_failures: Default::default(),
            failed_model: None,
            initial_discovery: None,
            completions: completions.into(),
            receipts: receipts.into(),
            keyed_receipts: std::collections::BTreeMap::new(),
            requests: vec![],
            calls: vec![],
            saved: Arc::new(Mutex::new(None)),
            fail_save: false,
            cancelled: false,
            notices: vec![],
            fail_notice: false,
        }
    }
}
#[async_trait]
impl Driver for Fixture {
    fn take_model_failure(&mut self) -> Option<exoharness_agent::FailedModel> {
        self.failed_model.take()
    }
    fn initial_discovery(&self) -> Option<&exoharness_agent::InitialDiscovery> {
        self.initial_discovery.as_ref()
    }
    fn cancelled(&self) -> bool {
        self.cancelled
    }
    async fn model(&mut self, _: &str, request: ChatRequest) -> Result<Completion, StopReason> {
        self.requests.push(request);
        if let Some(failure) = self.model_failures.remove(&self.requests.len()) {
            self.failed_model = Some(failure);
            return Err(StopReason::ModelIncomplete);
        }
        self.completions
            .pop_front()
            .ok_or(StopReason::ModelIncomplete)
    }
    async fn invoke(&mut self, key: &str, _: &ToolCall) -> Result<Receipt, StopReason> {
        self.calls.push(key.into());
        self.keyed_receipts
            .remove(key)
            .or_else(|| self.receipts.pop_front())
            .ok_or(StopReason::OutcomeUnknown)
    }
    async fn record_notice(&mut self, notice: serde_json::Value) -> Result<(), StopReason> {
        if self.fail_notice {
            return Err(StopReason::PersistenceFailed);
        }
        self.notices.push(notice);
        Ok(())
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
        max_recovery_tokens: 100_000,
        recovery_output_cap_available: true,
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

fn unavailable_service(id: &str) -> Receipt {
    let mut r = receipt(id, ReceiptState::RejectedBeforeExecution);
    r.performed_execution = false;
    r.result = json!({"isError":true,"_host_service_unavailable":1,"code_executed":false,"message":"MCP initialization unavailable"});
    r
}

#[tokio::test]
async fn discovery_projection_changes_only_after_activation_in_the_real_engine_loop() {
    let mut state = state();
    let initial_discovery = Some(exoharness_agent::InitialDiscovery::new(
        json!({"items":[{"kind":"capability","id":"provider","name":"Renderer","revision":1,
            "description":"RECOGNITION ONLY ".repeat(100)}]}),
        vec![],
    ));
    let activate = ToolCall {
        id: "select".into(),
        name: "activate_tools".into(),
        arguments: json!({"names":["registered-tool"]}),
    };
    let mut activation_receipt = receipt("selection", ReceiptState::Completed);
    activation_receipt.performed_execution = false;
    activation_receipt.result = json!({"executed":false,"loaded":["registered-tool"]});
    let mut driver = Fixture::new(
        vec![
            completion(vec![activate]),
            completion(vec![proposal("render")]),
            completion(vec![]),
        ],
        vec![
            activation_receipt,
            receipt("render", ReceiptState::Completed),
        ],
    );
    driver.initial_discovery = initial_discovery;
    let mut schemas = tools();
    schemas.push(Tool {
        name: "activate_tools".into(),
        description: "Activate exact tools".into(),
        input_schema: json!({"type":"object"}),
    });
    assert_eq!(
        run(
            &mut driver,
            &mut state,
            &policy(),
            "KEEP POLICY",
            &BTreeSet::new(),
            &schemas
        )
        .await,
        RunOutcome::Finished("done".into())
    );
    assert_eq!(driver.requests.len(), 3);
    for (index, request) in driver.requests.iter().enumerate() {
        let Message::System { text } = &request.messages[0] else {
            panic!("system message required")
        };
        assert!(text.starts_with("KEEP POLICY\n"));
        assert_eq!(text.contains("RECOGNITION ONLY"), index == 0);
        assert_eq!(
            serde_json::to_value(&request.tools).unwrap(),
            serde_json::to_value(&schemas).unwrap()
        );
    }
    assert_eq!(driver.calls.len(), 2);
    assert_eq!(state.model_calls, 3);
    assert!(
        !serde_json::to_value(&state)
            .unwrap()
            .to_string()
            .contains("RECOGNITION ONLY")
    );
    assert!(state.entries.iter().any(
        |entry| matches!(entry, JournalEntry::ToolExchange { tool, arguments, .. }
        if tool == "registered-tool" && arguments["native_array"] == json!([{"n":1}]))
    ));
}

#[tokio::test]
async fn unavailable_service_replans_without_replaying_successful_or_unsubmitted_calls() {
    let mut fallback = proposal("fallback");
    fallback.name = "fallback-tool".into();
    let mut available_tools = tools();
    let mut fallback_schema = available_tools[0].clone();
    fallback_schema.name = "fallback-tool".into();
    available_tools.push(fallback_schema);
    let mut driver = Fixture::new(
        vec![
            completion(vec![proposal("saved")]),
            completion(vec![proposal("unavailable"), proposal("dependent")]),
            completion(vec![fallback]),
            completion(vec![]),
        ],
        vec![
            receipt("saved", ReceiptState::Completed),
            unavailable_service("unavailable"),
            receipt("fallback", ReceiptState::Completed),
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
            &available_tools
        )
        .await,
        RunOutcome::Finished("done".into())
    );
    assert_eq!(
        driver.calls,
        vec!["model-1-tool-0", "model-2-tool-0", "model-3-tool-0"]
    );
    assert_eq!(state.service_recoveries, 1);
    assert_eq!(state.input_corrections, 0);
    assert_eq!(state.successful_executions, 2);
    let request = serde_json::to_string(&driver.requests[2]).unwrap();
    assert!(request.contains("MCP initialization unavailable"));
    assert!(request.contains("saved"));
}

#[tokio::test]
async fn service_unavailability_does_not_retry_same_tool_and_resume_keeps_decision() {
    let mut driver = Fixture::new(
        (0..3)
            .map(|n| completion(vec![proposal(&format!("failed-{n}"))]))
            .collect(),
        (0..3)
            .map(|n| unavailable_service(&format!("failed-{n}")))
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
        RunOutcome::Stopped(StopReason::UnchangedCorrection)
    );
    assert_eq!(state.service_recoveries, 1);
    let mut restored: EngineState =
        serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
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
    for (status, executed) in [
        (ReceiptState::OutcomeUnknown, false),
        (ReceiptState::FailedAfterExecution, false),
        (ReceiptState::RejectedBeforeExecution, true),
    ] {
        let mut r = unavailable_service("unsafe");
        r.state = status;
        r.performed_execution = executed;
        let mut driver = Fixture::new(vec![completion(vec![proposal("unsafe")])], vec![r]);
        let mut state = super_state();
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
        assert_eq!(state.service_recoveries, 0);
        assert_eq!(driver.requests.len(), 1);
    }
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
        (0..5)
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
    assert_eq!(driver.requests.len(), 5);
    assert!(driver.requests.last().unwrap().tools.is_empty());
    assert!(restored.repeat_finalization);
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

#[tokio::test]
async fn repeated_proposal_gets_one_tool_free_final_response_without_resubmission() {
    let mut completions: Vec<_> = (0..4)
        .map(|i| completion(vec![proposal(&format!("p{i}"))]))
        .collect();
    let mut final_answer = completion(vec![]);
    final_answer.text = "The task was submitted; completion has not yet been verified.".into();
    completions.push(final_answer);
    let mut driver = Fixture::new(
        completions,
        (0..3)
            .map(|i| receipt(&format!("host-{i}"), ReceiptState::Completed))
            .collect(),
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
    assert_eq!(driver.calls.len(), 3);
    assert_eq!(state.tool_calls, 3);
    assert!(driver.requests.last().unwrap().tools.is_empty());
    assert!(state.repeat_finalization);
    assert!(state.pending.is_empty());
    assert!(state.entries.iter().any(|entry| matches!(entry,
        JournalEntry::ToolExchange { result, .. }
        if result["code"] == "repeated_call_not_submitted" && result["performed_execution"] == false)));
    let mut restored: EngineState =
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
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
    assert_eq!(driver.requests.len(), 5);
    assert_eq!(driver.calls.len(), 3);
}

#[tokio::test]
async fn completed_media_can_be_retrieved_inline_then_finished_under_context_budget() {
    let mut submit = proposal("submit");
    submit.arguments = json!({"prompt":"existing fixture"});
    let mut inspect = proposal("inspect");
    inspect.arguments = json!({"job_id":"fixture-job","inline":true});
    let mut generated = receipt("generated", ReceiptState::Completed);
    generated.result = json!({"isError":false,"content":[{"type":"resource_link","uri":"https://example.test/logo.png","name":"logo.png"}],"structuredContent":{"id":"fixture-job","status":"succeeded"}});
    let mut inspected = receipt("retrieved", ReceiptState::Completed);
    inspected.result = generated.result.clone();
    inspected.result["content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"image","mimeType":"image/png","data":"A".repeat(3_311_980)}));
    let mut driver = Fixture::new(
        vec![
            completion(vec![submit]),
            completion(vec![inspect]),
            completion(vec![]),
        ],
        vec![generated, inspected],
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
    assert_eq!(driver.calls.len(), 2);
    assert_eq!(driver.requests.len(), 3);
    assert!(
        serde_json::to_vec(driver.requests.last().unwrap())
            .unwrap()
            .len()
            < policy().max_context_bytes
    );
    assert!(state.entries.iter().any(|entry|matches!(entry,JournalEntry::ToolExchange { result, .. } if result["content"][1]["data"].as_str().is_some_and(|data|data.len()==3_311_980))));
}

fn terminated_sandbox_failure(id: &str) -> Receipt {
    let mut r = receipt(id, ReceiptState::FailedAfterExecution);
    r.result = json!({"isError":true,"_host_sandbox_failure":1,"workspace_id":"saved-workspace","revision":5,"execution":{"status":"failed","cleanup_confirmed":true,"exit_code":1,"stderr":"network is disabled"}});
    r
}
#[tokio::test]
async fn terminated_sandbox_failure_replans_discards_stale_batch_and_preserves_success() {
    let mut corrected = proposal("corrected");
    corrected.arguments = json!({"revision":6,"input":"local.png"});
    let mut driver = Fixture::new(
        vec![
            completion(vec![proposal("generate")]),
            completion(vec![proposal("bad-code"), proposal("stale-publish")]),
            completion(vec![corrected]),
            completion(vec![]),
        ],
        vec![
            receipt("image", ReceiptState::Completed),
            terminated_sandbox_failure("failed-code"),
            receipt("fixed-code", ReceiptState::Completed),
        ],
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
    assert_eq!(
        driver.calls,
        vec!["model-1-tool-0", "model-2-tool-0", "model-3-tool-0"]
    );
    assert_eq!(state.sandbox_corrections, 1);
    assert_eq!(state.input_corrections, 0);
    assert_eq!(state.successful_executions, 2);
    let saved: EngineState = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert_eq!(saved.sandbox_corrections, 1);
}
#[tokio::test]
async fn sandbox_corrections_are_bounded_and_unknown_or_cancelled_work_never_replans() {
    let mut driver = Fixture::new(
        (0..3)
            .map(|i| completion(vec![proposal(&format!("p{i}"))]))
            .collect(),
        (0..3)
            .map(|i| terminated_sandbox_failure(&format!("failed-{i}")))
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
        RunOutcome::Stopped(StopReason::ToolFailed)
    );
    assert_eq!(state.sandbox_corrections, 2);
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
        RunOutcome::Stopped(StopReason::ToolFailed)
    );
    assert_eq!(driver.requests.len(), 3);
    for case in ["unknown", "cancelled", "cleanup-unconfirmed"] {
        let mut r = terminated_sandbox_failure("bad");
        match case {
            "unknown" => r.state = ReceiptState::OutcomeUnknown,
            "cancelled" => r.result["execution"]["status"] = json!("cancelled"),
            _ => r.result["execution"]["cleanup_confirmed"] = json!(false),
        }
        let mut d = Fixture::new(vec![completion(vec![proposal("p")])], vec![r]);
        let mut st = super_state();
        assert!(matches!(
            run(
                &mut d,
                &mut st,
                &policy(),
                "system",
                &BTreeSet::new(),
                &tools()
            )
            .await,
            RunOutcome::Stopped(_)
        ));
        assert_eq!(st.sandbox_corrections, 0);
        assert_eq!(d.requests.len(), 1);
    }
}

#[tokio::test]
async fn large_search_results_reach_video_execution_instead_of_exhausting_context() {
    let data = json!({"results":[{"url":"https://github.com/jason571/Exoharness","title":"ExoHarness","raw_content":"Rust harness source ".repeat(2400)}]});
    let mut search = receipt("search", ReceiptState::Completed);
    search.performed_execution = false;
    search.result = json!({"isError":false,"structuredContent":data,"content":[{"type":"text","text":data.to_string()}]});
    let mut call = proposal("search");
    call.name = "search-source".into();
    let mut video_call = proposal("video");
    video_call.name = "generate-video".into();
    video_call.arguments = json!({"duration_seconds":15});
    let mut video = receipt("video", ReceiptState::Completed);
    video.result = json!({"job_id":"fixture-video","status":"completed","url":"https://media.example/fixture.mp4"});
    let mut driver = Fixture::new(
        vec![
            completion(vec![call]),
            completion(vec![video_call]),
            completion(vec![]),
        ],
        vec![search.clone(), video],
    );
    let mut limits = policy();
    limits.max_context_bytes = 70_000;
    assert!(serde_json::to_vec(&search.result).unwrap().len() > limits.max_context_bytes);
    let mut schemas = tools();
    schemas[0].name = "search-source".into();
    let mut generate = schemas[0].clone();
    generate.name = "generate-video".into();
    schemas.push(generate);
    let mut state = EngineState::new(
        vec![JournalEntry::User(
            "Create a 15-second ExoHarness video using https://github.com/jason571/Exoharness"
                .into(),
        )],
        3,
    )
    .unwrap();
    assert!(matches!(
        run(
            &mut driver,
            &mut state,
            &limits,
            "system",
            &BTreeSet::new(),
            &schemas
        )
        .await,
        RunOutcome::Finished(_)
    ));
    assert_eq!(
        driver.calls.len(),
        2,
        "exactly one source lookup and one video submission"
    );
    assert_eq!(state.successful_executions, 1);
    assert!(
        driver
            .requests
            .iter()
            .all(|request| serde_json::to_vec(request).unwrap().len() < limits.max_context_bytes)
    );
    assert!(
        matches!(&state.entries[1],JournalEntry::ToolExchange{result,..} if result==&search.result)
    );
}
