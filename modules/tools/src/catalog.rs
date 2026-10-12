// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{CATALOG_ID, Result, SANDBOX_ID, ToolError, admission, native, public_name};
use exoharness_contracts::{
    capability::{CapabilitySpec, StoredCapability},
    execution::CapabilitySnapshot,
    model::Tool,
};
use exoharness_registry::Registry;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub struct Binding {
    pub capability: String,
    pub revision: u64,
    pub raw_name: String,
    pub tool: Tool,
}
pub struct Catalog {
    pub(crate) profile: Value,
    pub(crate) records: BTreeMap<String, StoredCapability>,
    pub(crate) bindings: BTreeMap<String, Binding>,
    pub(crate) active: BTreeSet<String>,
    pub(crate) read: BTreeSet<(String, String)>,
    pub(crate) discovery_key: Option<String>,
    execution_selected: bool,
    pub(crate) model_choices: BTreeMap<String, exoharness_model_runtime::SpeechModel>,
}
impl Catalog {
    /// Freeze authorized revisions and verified discovery, without connecting
    /// to every service or exposing every schema to every model request.
    pub fn load(
        registry: &Registry,
        access: &RegistryAccess,
        profile: Value,
        sandbox_ready: bool,
    ) -> Result<Self> {
        let views = registry
            .views(access, 1000)
            .map_err(|_| ToolError::Storage)?;
        let mut catalog = Self {
            profile,
            records: BTreeMap::new(),
            bindings: BTreeMap::new(),
            active: BTreeSet::new(),
            read: BTreeSet::new(),
            discovery_key: None,
            execution_selected: false,
            model_choices: BTreeMap::new(),
        };
        for view in views {
            let record = &view.capability;
            let kind = match record.manifest.spec {
                CapabilitySpec::Skill(_) => "skills",
                CapabilitySpec::McpServer(_) => "mcp",
                CapabilitySpec::ToolNode(_) => "tools",
            };
            if !admission::granted(&view) || !admission::allowed(&catalog.profile, kind, &record.id)
            {
                continue;
            }
            if let CapabilitySpec::McpServer(_) = record.manifest.spec {
                for value in registry
                    .0
                    .discovered_tools(access, &record.id)
                    .map_err(|_| ToolError::Storage)?
                {
                    let raw = value["name"].as_str().ok_or(ToolError::Storage)?;
                    let name = public_name(&record.id, raw);
                    let tool = Tool {
                        name: name.clone(),
                        description: value["description"].as_str().unwrap_or("").into(),
                        input_schema: value["inputSchema"].clone(),
                    };
                    catalog.bindings.insert(
                        name,
                        Binding {
                            capability: record.id.clone(),
                            revision: record.revision as u64,
                            raw_name: raw.into(),
                            tool,
                        },
                    );
                }
            }
            catalog.records.insert(record.id.clone(), record.clone());
        }
        if sandbox_ready && admission::allowed(&catalog.profile, "tools", SANDBOX_ID) {
            for tool in native::sandbox_tools() {
                catalog.bindings.insert(
                    tool.name.clone(),
                    Binding {
                        capability: SANDBOX_ID.into(),
                        revision: 1,
                        raw_name: tool.name.clone(),
                        tool,
                    },
                );
            }
        }
        Ok(catalog)
    }
    pub fn schemas(&self) -> Vec<Tool> {
        let mut tools = native::base();
        tools.extend(
            self.active
                .iter()
                .filter_map(|name| self.bindings.get(name).map(|b| b.tool.clone())),
        );
        tools
    }
    pub fn requires_execution(&self) -> bool {
        self.execution_selected
    }
    pub fn binding(&self, name: &str) -> Option<Binding> {
        self.bindings
            .get(name)
            .filter(|_| self.active.contains(name))
            .cloned()
            .or_else(|| {
                native::base()
                    .into_iter()
                    .find(|t| t.name == name)
                    .map(|tool| Binding {
                        capability: CATALOG_ID.into(),
                        revision: 1,
                        raw_name: name.into(),
                        tool,
                    })
            })
    }
    pub fn snapshots(&self) -> Vec<CapabilitySnapshot> {
        let mut snapshots: BTreeMap<String, CapabilitySnapshot> = BTreeMap::new();
        for binding in self
            .bindings
            .values()
            .cloned()
            .chain(native::base().into_iter().map(|tool| Binding {
                capability: CATALOG_ID.into(),
                revision: 1,
                raw_name: tool.name.clone(),
                tool,
            }))
        {
            snapshots
                .entry(binding.capability.clone())
                .or_insert_with(|| CapabilitySnapshot {
                    id: binding.capability.clone(),
                    revision: binding.revision,
                    grant_revision: self.profile["active_revision"].as_u64().unwrap_or(1),
                    allowed: true,
                    tool_schemas: BTreeMap::new(),
                })
                .tool_schemas
                .insert(binding.tool.name, binding.tool.input_schema);
        }
        // Skill revisions are frozen even though reading is dispatched by the
        // catalog provider. A package never becomes executable by this entry.
        for record in self
            .records
            .values()
            .filter(|r| matches!(r.manifest.spec, CapabilitySpec::Skill(_)))
        {
            snapshots
                .entry(record.id.clone())
                .or_insert_with(|| CapabilitySnapshot {
                    id: record.id.clone(),
                    revision: record.revision as u64,
                    grant_revision: self.profile["active_revision"].as_u64().unwrap_or(1),
                    allowed: true,
                    tool_schemas: BTreeMap::new(),
                });
        }
        snapshots.into_values().collect()
    }
    pub fn search(&self, query: &str, offset: usize) -> Value {
        self.search_kind(query, offset, None)
    }
    pub fn search_kind(&self, query: &str, offset: usize, kind: Option<&str>) -> Value {
        self.search_scoped(query, offset, kind, None, &[])
    }
    pub fn skill_candidates(&self, task: &str) -> Vec<Value> {
        if task.trim().is_empty() {
            return vec![];
        }
        self.search_kind(task, 0, Some("skill"))["items"]
            .as_array()
            .into_iter()
            .flatten()
            .take(3)
            .map(|v| {
                let mut v = v.clone();
                if let Some(description) = v["description"].as_str() {
                    v["description"] = json!(description.chars().take(480).collect::<String>());
                }
                v
            })
            .collect()
    }
    pub fn activate(&mut self, names: &[String]) -> Result<Value> {
        self.activate_with_mode(names, "replace")
    }
    /// Selection is atomic and never widens the frozen authorization catalog.
    /// Preserve historical replace semantics; explicit add avoids unloading
    /// an already selected dependency while adding a related stage tool.
    pub fn activate_with_mode(&mut self, names: &[String], mode: &str) -> Result<Value> {
        // Known native operations absent from the authorized bindings are a
        // permission/readiness denial, not a typo that model retries can fix.
        let native_tools = native::sandbox_tools();
        if names.iter().any(|name| {
            !self.bindings.contains_key(name) && native_tools.iter().any(|tool| &tool.name == name)
        }) {
            return Err(ToolError::Authorization);
        }
        if !matches!(mode, "add" | "replace") {
            return Err(ToolError::Input(
                "mode 必须为 add 或 replace；未改变加载集合".into(),
            ));
        }
        if names.len() > 32 {
            return Err(ToolError::Input(
                "只能加载能力查询返回的已授权工具名称；未改变加载集合".into(),
            ));
        }
        let mut resolved = crate::selection::resolve(&self.bindings, names, mode)?;
        if mode == "add" {
            resolved.extend(self.active.iter().cloned());
        }
        if resolved.len() > 32 {
            return Err(ToolError::Input(
                "加载集合超过32个工具；未改变加载集合，请用 replace 选择当前步骤所需工具".into(),
            ));
        }
        self.active = resolved;
        self.execution_selected |= !names.is_empty();
        Ok(json!({"loaded":self.active,"mode":mode,"executed":false}))
    }
    pub fn execution_guidance(&self) -> Value {
        let workspace_tools: Vec<_> = self
            .bindings
            .values()
            .filter(|b| b.capability == SANDBOX_ID)
            .map(|b| &b.tool.name)
            .collect();
        json!({
            "message":"Skill instructions do not define the installed execution interface. Select a relevant capability from the authorized service index, then discover only that capability's tools and activate the current stage's exact names. For names declared in a Skill, use a single names lookup. Never activate a Skill ID or enumerate all providers' tools to recover a lexical miss.",
            "discover":{"tool":"search_capabilities","arguments":{"query":"","kind":"capability"}},
            "workspace_tools":workspace_tools,
            "workspace_available":!workspace_tools.is_empty(),
            "native_workspace_contract":if workspace_tools.is_empty(){Value::Null}else{json!({
                "authority":"Use these installed interfaces and their live schemas when imported Skill instructions describe another host. This guidance does not grant access or change Skill content.",
                "create":"workspace_create with optional skill_id binds the authorized package once; save files with workspace_edit using the returned workspace_id and revision.",
                "execute":"sandbox_run reuses workspace_id and expected_revision; omit skill_id and files. The workspace already retains its immutable Skill binding. Do not create a separate workspace to check existing inputs.",
                "runtime":"Use runtime=javascript for Node .js/.mjs/.cjs scripts (not node or python); python for .py, shell for .sh, typescript for .ts/.mts/.cts.",
                "skill_entrypoint":"For a packaged script use entrypoint=skill/<package-relative-path>, not an absolute path. Script arguments refer to saved task files in the working directory.",
                "legacy_names":"edit_workspace maps to workspace_edit; run_script maps to sandbox_run; runtime_name=node maps to runtime=javascript; entrypoint_root=skill maps to the skill/ entrypoint prefix; outputs maps to output_files. Verify the live schema before submission.",
                "publish":"output_files selects final files for publication; workspace_publish can publish existing outputs without rerunning code. Do not install dependencies or execute on the host."
            })},
            "coding_workflow":if workspace_tools.is_empty(){Value::Null}else{json!({"tools":["coding_search","coding_apply_patch","coding_check","coding_publish"],"message":"Reuse the saved project; search and read only related files. Apply atomic focused patches, run saved checks in the OCI sandbox, correct failing source without rewriting passing files, then publish final outputs. Check success is not final delivery. Browser acceptance requires an actual authorized browser or prepared sandbox browser; do not claim it from a successful build. Default network is disabled and missing dependencies require administrator preparation."})}
        })
    }
}

#[cfg(test)]
#[path = "../tests/unit/catalog.rs"]
mod tests;
