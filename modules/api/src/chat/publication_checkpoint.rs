//! Native, committed execution evidence separates computation from delivery.
//! External MCP metadata cannot opt into this recovery boundary.
use serde_json::{Value, json};

pub(super) fn project(summary: &mut Value, receipt: &Value) {
    let pending = receipt["_host_publication_pending"] == true;
    let complete = receipt["_host_publication_complete"] == true;
    if !pending && !complete {
        return;
    }
    let (Some(id), Some(revision)) = (
        receipt["workspace_id"].as_str(),
        receipt["revision"].as_u64(),
    ) else {
        return;
    };
    if pending && receipt["execution"]["exit_code"] != 0 {
        return;
    }
    if complete && receipt["artifacts"].as_array().is_none_or(Vec::is_empty) {
        return;
    }
    // Workspace scope keeps independent outputs from closing each other's
    // obligations. Full source/results stay in the execution journal.
    summary["_host_workflow"] = json!({
        "version":1,
        "capability":format!("{}:publication:{id}", exoharness_tools::SANDBOX_ID),
        "revision":1,
        "directive":{
            "version":1,"context_scope":"checkpoint",
            "status":if pending {"required"} else {"complete"},
            "required_tool":"workspace_publish",
            "required_arguments":{"workspace_id":id,"expected_revision":revision},
            "checkpoint":{"workspace_id":id,"revision":revision}
        },
        "tools":if pending {vec!["workspace_publish", "workspace_read"]} else {vec![]},
        "provider_tools":[]
    });
    summary["_publication_recovery_only"] = json!(pending);
}

pub(super) fn restrict(
    entries: &[exoharness_core::context::JournalEntry],
    invocation: &str,
    required: bool,
) -> bool {
    required
        && entries.iter().any(|entry| {
            matches!(entry,
        exoharness_core::context::JournalEntry::Checkpoint{invocation_id,summary,..}
        if invocation_id == invocation && summary["_publication_recovery_only"] == true)
        })
}
