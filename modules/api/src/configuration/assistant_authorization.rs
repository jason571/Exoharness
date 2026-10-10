//! A saved selection is not an execution grant. Recheck all layers at dispatch.
use super::ConfigurationState;
use exoharness_configuration::{Assistants, ConfigurationError};
use exoharness_contracts::capability::{CapabilityKind, CapabilityScope};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::Value;
pub(super) fn validate(
    state: &ConfigurationState,
    id: Option<&str>,
    input: &Value,
) -> Result<(), ConfigurationError> {
    let previous = id.and_then(|id| state.assistants.detail(id));
    let access = input.get("access").filter(|v| !v.is_null()).or_else(|| {
        previous
            .as_ref()
            .and_then(|p| p.get("access"))
            .filter(|v| !v.is_null())
    });
    for (group, kind) in [
        ("skills", CapabilityKind::Skill),
        ("mcp", CapabilityKind::McpServer),
        ("tools", CapabilityKind::ToolNode),
    ] {
        let ids = access
            .and_then(|a| a.get(group))
            .filter(|s| s["mode"] == "selected")
            .and_then(|s| s["ids"].as_array())
            .or_else(|| {
                if group == "skills" && access.is_none() {
                    input["skill_ids"].as_array()
                } else {
                    None
                }
            });
        for id in ids.into_iter().flatten() {
            let unchanged = previous
                .as_ref()
                .and_then(|p| p.pointer(&format!("/access/{group}")))
                .is_some_and(|scope| {
                    scope["mode"] == "selected"
                        && scope["ids"].as_array().is_some_and(|old| old.contains(id))
                });
            // Imported snapshots may retain unavailable references. Preserve
            // unchanged selections while validating every NEW grant. Execution
            // still requires a current enabled, authorized registry record.
            if unchanged {
                continue;
            }
            registered(
                state,
                id.as_str()
                    .ok_or(ConfigurationError::Invalid("invalid capability ID"))?,
                &[kind],
            )?;
        }
    }
    let models = state.models.refresh()?;
    for name in access
        .and_then(|a| a.pointer("/models/ids"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !models["models"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|m| m["name"] == *name && m["model_type"] != "llm")
        {
            return Err(ConfigurationError::Invalid(
                "所选模型能力未登记或属于LLM；请将对话模型与执行能力分开配置",
            ));
        }
    }
    if let Some(name) = input.get("default_llm").filter(|v| !v.is_null()) {
        let name = name.as_str().ok_or(ConfigurationError::Invalid(
            "默认对话模型需要模型名称或null",
        ))?;
        if !previous.as_ref().is_some_and(|p| p["default_llm"] == name) {
            let binding = state.models.bind_enabled(name)?;
            if binding.model["model_type"].as_str().unwrap_or("llm") != "llm" {
                return Err(ConfigurationError::Invalid(
                    "默认对话模型只能选择已启用的LLM",
                ));
            }
        }
    }
    if let Some(id) = access
        .and_then(|a| a.pointer("/software_engineering/runtime_capability_id"))
        .and_then(Value::as_str)
    {
        registered(
            state,
            id,
            &[CapabilityKind::McpServer, CapabilityKind::ToolNode],
        )?;
    }
    Ok(())
}
fn registered(
    state: &ConfigurationState,
    id: &str,
    kinds: &[CapabilityKind],
) -> Result<(), ConfigurationError> {
    // Built-in execution is an explicitly selectable provider, not an imported
    // MCP and not a grant inherited from a Skill. Runtime readiness is checked
    // independently when building the authorized execution catalog.
    if id == exoharness_tools::SANDBOX_ID && kinds.contains(&CapabilityKind::ToolNode) {
        return Ok(());
    }
    let registry = state.registry.as_ref().ok_or(ConfigurationError::Invalid(
        "capability registry is not connected",
    ))?;
    let record = registry
        .0
        .capability(&RegistryAccess::Administrator, id)
        .map_err(|error| match error {
            exoharness_storage_postgres::StoreError::NotFound => {
                ConfigurationError::Invalid("selected capability is not registered")
            }
            _ => ConfigurationError::PrivateFile,
        })?;
    if !kinds.contains(&record.manifest.kind())
        || !matches!(record.manifest.scope, CapabilityScope::Global)
    {
        return Err(ConfigurationError::Invalid(
            "capability kind or sharing scope does not allow assistant assignment",
        ));
    }
    // Disabled registrations remain visible as unavailable; assignment never enables them.
    Ok(())
}
pub(super) fn referenced(state: &ConfigurationState, id: &str) -> Result<bool, ConfigurationError> {
    let identity = state.identity.as_ref().ok_or(ConfigurationError::Invalid(
        "user assignments are not connected; deletion is unavailable",
    ))?;
    identity
        .assistant_reference_count(id)
        .map(|n| n > 0)
        .map_err(|_| ConfigurationError::PrivateFile)
}
pub(super) fn current(assistants: &Assistants, id: &str) -> Result<Value, ConfigurationError> {
    assistants.detail(id).ok_or(ConfigurationError::NotFound)
}
