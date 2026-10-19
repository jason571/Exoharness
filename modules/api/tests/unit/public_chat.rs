use super::*;

#[test]
fn model_response_diagnostics_remain_in_trace() {
    assert!(event(json!({"type":"model_response_correction","diagnostic":"tool_arguments_json","usage":{"total_tokens":123}})).is_none());
}

#[test]
fn partial_answer_is_visible_without_publishing_private_failure_fields() {
    let value=event(json!({"type":"failed","message":"private stack","public_notice":{"code":"task_processing_limited"},"response":{"answer":"13×24=312，动画暂时不可用。","partial_result":true,"model_error":{"detail":"private stack"},"usage":{"total_tokens":88}}})).unwrap();
    assert_eq!(value["response"]["partial_result"], true);
    assert!(
        value["response"]["answer"]
            .as_str()
            .unwrap()
            .contains("312")
    );
    assert!(!value.to_string().contains("private stack"));
    assert_eq!(value["status"], "failed");
}

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
        "mcp_background_reconciliation",
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

#[test]
fn normal_progress_messages_and_exact_numbers_survive_public_projection() {
    for message in [
        "[sampling] ComfyUI 节点 10 采样进度 7/8",
        "正在生成视频，已运行 125 秒",
        "Preparing references",
        "Encoding video · elapsed: 95 seconds",
        "正在排队，前面还有 2 个任务",
    ] {
        let original = json!({"type":"tool_progress","tool_call_id":"owned-call","progress":84.25,"total":100.0,"message":message});
        let projected = event(original.clone()).unwrap();
        assert_eq!(projected, original);
    }
}

#[test]
fn unsafe_progress_is_hidden_without_suppressing_real_percentage_or_trace() {
    for message in [
        "Traceback: RuntimeError",
        "MCP 503",
        "permission denied",
        "任务执行失败",
        "API_KEY=private",
        "Authorization: Bearer private",
        "saved /home/private/output.mp4",
        "download https://example.test/file?signature=private",
        "Exception at node",
        "error: failed",
        "{\"token\":\"private\"}",
    ] {
        let original = json!({"type":"tool_progress","tool_call_id":"owned-call","progress":30,"total":100,"message":message,"diagnostics":"private extension","token":"private"});
        let projected = event(original.clone()).unwrap();
        assert_eq!(projected["message"], "正在处理...");
        assert_eq!(projected["progress"], 30);
        assert_eq!(projected["total"], 100);
        assert!(projected.get("diagnostics").is_none());
        assert!(projected.get("token").is_none());
        assert_eq!(original["message"], message);
    }
}

#[test]
fn missing_empty_or_invalid_messages_keep_indeterminate_and_numeric_progress() {
    for message in [
        Value::Null,
        json!(""),
        json!("   "),
        json!(123),
        json!("X".repeat(4097)),
        json!("normal\nTraceback"),
    ] {
        let projected = event(
            json!({"type":"tool_progress","tool_call_id":"id","progress":7,"message":message}),
        )
        .unwrap();
        assert_eq!(projected["progress"], 7);
        assert_eq!(projected["total"], Value::Null);
        assert_eq!(projected["message"], "正在处理...");
    }
}
