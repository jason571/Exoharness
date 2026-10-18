use super::*;

#[test]
fn a_failed_task_survives_a_legacy_successful_clarification_to_continue() {
    let history = json!({"turns":[{"run_id":"original","run_status":"failed","prompt":"琪琪的乘法题"},{"run_id":"clarification","run_status":"succeeded","prompt":"继续生成","response":{"answer":"What should I generate?","tool_calls":[],"artifacts":[]}}]});
    assert_eq!(continuation_run(&history, "继续生成"), Some("original"));
    assert_eq!(continuation_run(&history, "continue"), Some("original"));
    assert_eq!(continuation_run(&history, "a new unrelated task"), None);
}
#[test]
fn successful_delivery_cancel_and_unrelated_new_turn_stop_continuation_lookup() {
    for latest in [
        json!({"run_status":"succeeded","prompt":"继续生成","response":{"artifacts":[{"filename":"final.mp4"}]}}),
        json!({"run_status":"succeeded","prompt":"继续生成","response":{"tool_calls":["render"]}}),
        json!({"run_status":"succeeded","prompt":"a different question","response":{"tool_calls":[],"artifacts":[]}}),
        json!({"run_status":"cancelled","prompt":"继续生成"}),
        json!({"run_status":"running","prompt":"继续生成"}),
    ] {
        assert_eq!(
            continuation_run(
                &json!({"turns":[{"run_status":"failed","run_id":"old"},latest]}),
                "继续生成"
            ),
            None
        );
    }
    let mut turns = vec![json!({"run_status":"failed","run_id":"too-old"})];
    turns.extend((0..8).map(|_|json!({"run_status":"succeeded","prompt":"继续生成","response":{"tool_calls":[],"artifacts":[]}})));
    assert_eq!(continuation_run(&json!({"turns":turns}), "继续生成"), None);
}
