use super::*;

#[test]
fn processing_limitation_does_not_claim_missing_permissions_and_keeps_artifacts() {
    let result = event(json!({"type":"failed","message":"private traceback","public_notice":{"code":"task_processing_limited"},"artifacts":[{"filename":"draft.json"}]})).unwrap();
    assert_eq!(result["public_notice"]["code"], "task_processing_limited");
    assert!(!result.to_string().contains("private traceback"));
    assert!(!result.to_string().contains("管理员"));
    assert_eq!(result["artifacts"][0]["filename"], "draft.json");
}

#[test]
fn raw_terminal_diagnostics_and_forged_messages_never_reach_chat() {
    for raw in [
        "Traceback: RuntimeError",
        "MCP 503",
        "执行已停止；请查看追踪",
        "permission denied",
    ] {
        let original = json!({"type":"failed","message":raw,"diagnostics":raw,"artifacts":[{"filename":"saved.png"}],"public_notice":{"code":"capability_limited","message":raw}});
        let projected = event(original.clone()).unwrap();
        assert!(!projected.to_string().contains(raw));
        assert_eq!(projected["artifacts"], original["artifacts"]);
        assert_eq!(original["message"], raw);
    }
}
#[test]
fn history_projection_hides_legacy_errors_and_keeps_original_trace_facts() {
    let original = json!({"turns":[{"run_status":"failed","run_error":"private diagnostic","response":{"model_error":{"kind":"Http","http_status":503},"artifacts":[{"filename":"saved.mp4"}],"usage":{"total_tokens":99}}}]});
    let mut projected = original.clone();
    history(&mut projected);
    assert!(!projected.to_string().contains("private diagnostic"));
    assert!(
        projected["turns"][0]["response"]
            .get("model_error")
            .is_none()
    );
    assert_eq!(
        projected["turns"][0]["response"]["artifacts"],
        original["turns"][0]["response"]["artifacts"]
    );
    assert_eq!(
        projected["turns"][0]["response"]["usage"],
        original["turns"][0]["response"]["usage"]
    );
}
#[test]
fn trace_only_events_are_hidden_and_progress_numbers_remain_real() {
    for kind in [
        "execution_exception",
        "input_rejected",
        "upstream_error",
        "tool_output",
    ] {
        assert!(event(json!({"type":kind,"message":"private"})).is_none());
    }
    let value =
        event(json!({"type":"tool_progress","progress":4,"total":8,"message":"RuntimeError"}))
            .unwrap();
    assert_eq!(value["progress"], 4);
    assert_eq!(value["total"], 8);
    assert!(!value.to_string().contains("RuntimeError"));
}
