use super::failure_notice_code;
use exoharness_agent::StopReason;
#[test]
fn exhausted_execution_budgets_are_not_reported_as_missing_tools_or_permissions() {
    for reason in [
        StopReason::TokenBudget,
        StopReason::ContextBudget,
        StopReason::RecoveryTokenBudget,
        StopReason::ModelBudget,
        StopReason::ToolBudget,
    ] {
        let code = failure_notice_code(Some(&reason), false, false, false);
        assert_eq!(code, "task_scope_limited");
        assert!(
            !crate::public_chat::notice(code)
                .to_string()
                .contains("管理员")
        );
    }
    assert_eq!(
        failure_notice_code(Some(&StopReason::InputRejected), true, false, false),
        "capability_permission_required"
    );
    assert_eq!(
        failure_notice_code(Some(&StopReason::InputRejected), false, false, true),
        "service_unavailable"
    );
    assert_eq!(
        failure_notice_code(Some(&StopReason::ModelIncomplete), false, true, false),
        "service_unavailable"
    );
}
