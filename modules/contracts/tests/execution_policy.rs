use exoharness_contracts::{invocation::InvocationState, retry::ConsecutiveCalls};
use serde_json::json;

#[test]
fn consecutive_limit_is_not_a_total_call_limit() {
    let mut guard = ConsecutiveCalls::new(3).unwrap();
    for _ in 0..3 {
        assert!(guard.admit("fetch", &json!({"page":1})));
    }
    assert!(!guard.admit("fetch", &json!({"page":1})));
    assert!(!guard.admit("fetch", &json!({"page":1})));
    assert!(guard.admit("fetch", &json!({"page":2})));
    assert!(guard.admit("render", &json!({"page":2})));
    assert!(guard.admit("fetch", &json!({"page":1})));
}

#[test]
fn argument_object_key_order_does_not_evade_limit() {
    let mut guard = ConsecutiveCalls::new(1).unwrap();
    assert!(guard.admit("tool", &serde_json::from_str("{\"a\":1,\"b\":2}").unwrap()));
    assert!(!guard.admit("tool", &serde_json::from_str("{\"b\":2,\"a\":1}").unwrap()));
    assert!(ConsecutiveCalls::new(0).is_err());
}

#[test]
fn known_success_and_unknown_outcomes_cannot_be_resubmitted() {
    for state in [
        InvocationState::Succeeded,
        InvocationState::Submitted,
        InvocationState::OutcomeUnknown,
        InvocationState::FailedAfterExecution,
    ] {
        assert!(!state.may_submit());
    }
    assert!(InvocationState::Prepared.may_submit());
    assert!(InvocationState::RejectedBeforeExecution.may_submit());
}
