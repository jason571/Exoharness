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

#[test]
fn large_call_repeat_guard_serializes_as_fingerprint_and_legacy_guard_keeps_limit() {
    let arguments = serde_json::json!({"source":"large authored code ".repeat(10000)});
    let mut guard = ConsecutiveCalls::new(3).unwrap();
    for _ in 0..3 {
        assert!(guard.admit("edit", &arguments));
    }
    let encoded = serde_json::to_vec(&guard).unwrap();
    assert!(encoded.len() < 500);
    let mut restored: ConsecutiveCalls = serde_json::from_slice(&encoded).unwrap();
    assert!(!restored.admit("edit", &arguments));
    let legacy = serde_json::json!({"limit":3,"previous":["edit",arguments],"count":3});
    let mut old: ConsecutiveCalls = serde_json::from_value(legacy).unwrap();
    assert!(!old.admit("edit", &arguments));
    let upgraded = serde_json::to_vec(&old).unwrap();
    assert!(upgraded.len() < 500);
    let mut old_again: ConsecutiveCalls = serde_json::from_slice(&upgraded).unwrap();
    assert!(!old_again.admit("edit", &arguments));
}
#[test]
fn repeat_fingerprint_preserves_native_json_equality_and_numeric_types() {
    let mut guard = ConsecutiveCalls::new(1).unwrap();
    assert!(guard.admit(
        "tool",
        &serde_json::from_str(r#"{"b":2,"a":-0.0}"#).unwrap()
    ));
    assert!(!guard.admit("tool", &serde_json::from_str(r#"{"a":0.0,"b":2}"#).unwrap()));
    assert!(guard.admit("tool", &serde_json::json!({"a":0,"b":2})));
    assert!(guard.admit("other", &serde_json::json!({"a":0,"b":2})));
}

#[test]
fn legacy_repeat_cursor_can_be_saved_before_next_admission_without_losing_guard() {
    let raw = serde_json::json!({"limit":1,"previous":["tool",{"n":1}],"count":1});
    let old: ConsecutiveCalls = serde_json::from_value(raw).unwrap();
    let encoded = serde_json::to_vec(&old).unwrap();
    let mut restored: ConsecutiveCalls = serde_json::from_slice(&encoded).unwrap();
    assert!(!restored.admit("tool", &serde_json::json!({"n":1})));
}
