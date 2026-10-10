// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_contracts::capability::{PermissionDecision, RegisteredCapabilityView};
use serde_json::Value;
/// Absence is denial; a Skill selection never grants MCP or script execution.
pub fn allowed(profile: &Value, kind: &str, id: &str) -> bool {
    let scope = &profile["access"][kind];
    scope["mode"] == "all"
        || (scope["mode"] == "selected"
            && scope["ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|v| v == id)))
}
pub(crate) fn granted(view: &RegisteredCapabilityView) -> bool {
    view.capability.enabled
        && view.capability.manifest.permissions.iter().all(|name| {
            view.permissions
                .iter()
                .any(|p| &p.permission == name && p.decision == PermissionDecision::Allow)
        })
}
