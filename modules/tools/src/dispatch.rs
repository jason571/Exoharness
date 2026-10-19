// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Catalog, DispatchResult, Result, SANDBOX_ID, ToolError, ToolRuntime, validate};
use exoharness_contracts::model::ToolCall;
use exoharness_mcp::{Connection, ConnectionOptions, McpError, Progress};
use exoharness_model_runtime::CancellationToken;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::json;
use tokio::sync::watch;

impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch(
        &self,
        catalog: &mut Catalog,
        owner: &str,
        session: &str,
        run: &str,
        invocation: &str,
        call: &ToolCall,
        cancel: CancellationToken,
        progress: watch::Sender<Option<Progress>>,
    ) -> Result<DispatchResult> {
        let binding = catalog
            .binding(&call.name)
            .ok_or(ToolError::Authorization)?;
        validate(&binding.tool.input_schema, &call.arguments)?;
        if cancel.is_cancelled() {
            return Err(ToolError::Input("运行已取消；未执行".into()));
        }
        let mut executed = false;
        let mut delivered = None;
        let mut value = match call.name.as_str() {
            "search_capabilities" => catalog.discover(&call.arguments)?,
            "activate_tools" => {
                let names: Vec<String> = serde_json::from_value(call.arguments["names"].clone())
                    .map_err(|_| ToolError::Input("names 必须为数组".into()))?;
                catalog.activate_with_mode(
                    &names,
                    call.arguments["mode"].as_str().unwrap_or("replace"),
                )?
            }
            "read_skill" => {
                let load: Vec<String> = call
                    .arguments
                    .get("load_tools")
                    .map(|names| serde_json::from_value(names.clone()))
                    .transpose()
                    .map_err(|_| {
                        ToolError::Input("load_tools must contain executable names".into())
                    })?
                    .unwrap_or_default();
                // Validate before reading or caching instructions. A rejected
                // selection must not make the first root read disappear.
                catalog.selection(&load, "add")?;
                let id = call.arguments["id"]
                    .as_str()
                    .ok_or(ToolError::Authorization)?;
                if !catalog.records.get(id).is_some_and(|record| {
                    matches!(
                        &record.manifest.spec,
                        exoharness_contracts::capability::CapabilitySpec::Skill(_)
                    )
                }) {
                    // A read lookup cannot distinguish an invented, stale or
                    // inaccessible ID outside the frozen authorized catalog.
                    // Correct metadata selection without disclosing existence;
                    // current() still enforces live grants and revisions below.
                    return Err(ToolError::CorrectableInput(
                        "read_skill.id must be an exact Skill ID from the authorized instructions catalog, not a service/tool ID. Use search_capabilities with kind=skill and no capability_id, then read a returned exact ID. Do not repeat this identity or infer task-wide missing permission. No package content was read or execution submitted.".into(),
                    ));
                }
                let record = self.current(catalog, owner, id).await?;
                let path = call.arguments["path"].as_str().unwrap_or("SKILL.md");
                // A retrieval hint must not invalidate an ordinary immutable
                // file read. Root and explicit reference reads retain their
                // complete content and canonical cache identity. Only the
                // declared index uses query to select additional resources.
                let query = call.arguments["query"]
                    .as_str()
                    .filter(|_| path == crate::skill_resources::INDEX);
                let read_key = query.map_or_else(
                    || path.to_owned(),
                    |q| {
                        use sha2::{Digest, Sha256};
                        format!("{path}?query={:x}", Sha256::digest(q.as_bytes()))
                    },
                );
                let mut value = if catalog.read.contains(&(id.into(), read_key.clone())) {
                    json!({"id":id,"revision":record.revision,"path":path,"already_loaded":true,"message":"Exact content remains in the preceding tool result; do not read it again.","execution_guidance":catalog.execution_guidance()})
                } else {
                    let skills = self.skills.clone();
                    let access = RegistryAccess::User(owner.into());
                    let package_id = id.to_owned();
                    let package = tokio::task::spawn_blocking(move || {
                        skills.package_snapshot(&access, &package_id, record.revision)
                    })
                    .await
                    .map_err(|_| ToolError::Storage)?
                    .map_err(|_| ToolError::Authorization)?;
                    let bytes = package.files.get(path).ok_or_else(|| {
                        ToolError::CorrectableInput("The requested relative file is not in this authorized Skill package. Read SKILL.md for its supported references; do not repeat this missing path or infer task-wide missing permissions. No execution was submitted.".into())
                    })?;
                    if bytes.len() > 256 * 1024 {
                        return Err(ToolError::CorrectableInput(
                            "文件过大，请先选择较小的参考文件；未将二进制或大文件发送给模型".into(),
                        ));
                    }
                    let text = std::str::from_utf8(bytes).map_err(|_| {
                        ToolError::CorrectableInput("This package file is not a UTF-8 instruction resource. Read SKILL.md for the supported file workflow; do not repeat a binary read. No execution was submitted.".into())
                    })?;
                    let resources = query
                        .map(|q| crate::skill_resources::select(&package.files, q))
                        .transpose()?;
                    let context = if path == "SKILL.md" {
                        crate::skill_context::compile(
                            catalog,
                            id,
                            record.revision,
                            text,
                            &package.files,
                        )
                    } else {
                        None
                    };
                    let mut combined = load.clone();
                    if let Some(context) = &context
                        && context["auto_activate_entry"] == true
                    {
                        let names: Vec<String> =
                            serde_json::from_value(context["entry_tools"].clone())
                                .map_err(|_| ToolError::Storage)?;
                        combined.extend(names);
                    }
                    let selected = catalog.selection(&combined, "add")?;
                    let mut value = json!({"id":id,"revision":record.revision,"path":path,"content":text,"files":package.files.keys().collect::<Vec<_>>(),"execution_guidance":catalog.execution_guidance()});
                    if let Some(resources) = resources {
                        // Keep the canonical root untouched; this separate resource read
                        // carries complete selected files and their immutable hashes.
                        value["content"] = json!(resources.to_string());
                        value["selected_resources"] = json!(
                            resources["resources"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|r| {
                                    json!({"id":r["id"],"path":r["path"],"sha256":r["sha256"]})
                                })
                                .collect::<Vec<_>>()
                        );
                        value.as_object_mut().unwrap().remove("files");
                    }
                    catalog.active = selected;
                    catalog.read.insert((id.into(), read_key));
                    if let Some(context) = context {
                        value["_host_skill_context"] = context;
                    }
                    value
                };
                if !load.is_empty() {
                    value["tool_selection"] = catalog.activate_with_mode(&load, "add")?;
                }
                value
            }
            _ if binding.capability == SANDBOX_ID => {
                if call.name.starts_with("coding_") {
                    return self
                        .coding_operation(
                            catalog,
                            owner,
                            session,
                            run,
                            invocation,
                            &call.name,
                            &call.arguments,
                            cancel,
                        )
                        .await;
                }
                if call.name != "sandbox_run" {
                    return self
                        .workspace_operation(
                            catalog,
                            owner,
                            session,
                            run,
                            invocation,
                            &call.name,
                            &call.arguments,
                        )
                        .await;
                }
                return self
                    .run_sandbox(
                        catalog,
                        owner,
                        session,
                        run,
                        invocation,
                        &call.arguments,
                        cancel,
                    )
                    .await;
            }
            _ if binding.capability.starts_with("tool:model:") => {
                return self
                    .run_model(
                        catalog,
                        owner,
                        session,
                        run,
                        &call.arguments,
                        &binding,
                        cancel,
                    )
                    .await;
            }
            _ => {
                let record = self.current(catalog, owner, &binding.capability).await?;
                let access = RegistryAccess::User(owner.into());
                let registry = self.registry.clone();
                let permission_id = record.id.clone();
                let permissions = tokio::task::spawn_blocking(move || {
                    registry.0.capability_permissions(&access, &permission_id)
                })
                .await
                .map_err(|_| ToolError::Storage)?
                .map_err(|_| ToolError::Storage)?;
                let connection = Connection::connect(
                    &record,
                    &permissions,
                    &self.mcp.secrets,
                    ConnectionOptions::default(),
                )
                .await
                .map_err(|error| match error {
                    McpError::Connection => {
                        ToolError::UnavailableBeforeExecution("MCP连接暂时不可用；尚未调用工具")
                    }
                    _ => ToolError::Input("MCP连接配置或授权不符合要求；尚未调用工具".into()),
                })?;
                let operation = async {
                    let tools = connection.tools().await.map_err(|error| match error {
                        McpError::Connection => ToolError::UnavailableBeforeExecution(
                            "MCP工具发现暂时不可用；尚未调用工具",
                        ),
                        _ => ToolError::Input("MCP工具发现协议不符合要求；尚未调用工具".into()),
                    })?;
                    let live = tools
                        .iter()
                        .find(|v| v["name"] == binding.raw_name)
                        .ok_or(ToolError::Authorization)?;
                    if live["inputSchema"] != binding.tool.input_schema {
                        return Err(ToolError::UnavailableBeforeExecution(
                            "MCP Schema已变化；当前固定工具版本不可执行，未提交业务调用。保留原始数据并使用当前已授权的其他接口；管理员可重新验证服务供后续新运行使用",
                        ));
                    }
                    validate(&live["inputSchema"], &call.arguments)?;
                    // Permissions can change while initialize/tools/list awaits.
                    self.current(catalog, owner, &binding.capability).await?;
                    let result = connection
                        .call(
                            &binding.raw_name,
                            call.arguments.clone(),
                            |update| {
                                progress.send_replace(Some(update));
                            },
                            cancel.cancelled(),
                        )
                        .await
                        .map_err(|error| match error {
                            McpError::OutcomeUnknown | McpError::Cancelled | McpError::Protocol => {
                                ToolError::Unknown
                            }
                            _ => ToolError::Input("MCP拒绝了调用；原始协议记录保留在服务端".into()),
                        })?;
                    let mut value = self
                        .observe_mcp(
                            catalog,
                            owner,
                            session,
                            run,
                            invocation,
                            &binding.capability,
                            &connection,
                            &tools,
                            result,
                            &cancel,
                            &progress,
                        )
                        .await?;
                    // These top-level receipts can only be authored by the
                    // host. Preserve all actual MCP business fields.
                    if let Some(object) = value.as_object_mut() {
                        object.remove("_host_delivered_artifacts");
                        object.remove("_host_saved_artifacts");
                    }
                    delivered = Some(
                        self.deliver_mcp_outputs(
                            catalog,
                            owner,
                            session,
                            run,
                            &binding.capability,
                            &cancel,
                            &value,
                        )
                        .await,
                    );
                    Ok(value)
                }
                .await;
                let _ = connection.close().await;
                executed = true;
                operation?
            }
        };
        if let Some(outputs) = &delivered {
            // Preserve the exact MCP fields; host delivery receipts are
            // separate metadata, not changes to the service result.
            if !outputs.cards.is_empty() {
                value["_host_delivered_artifacts"] = json!(outputs.cards);
            }
            if !outputs.saved.is_empty() {
                value["_host_saved_artifacts"] = json!(outputs.saved);
            }
        }
        Ok(DispatchResult {
            failed: value["isError"] == true,
            artifacts: delivered
                .map(|outputs| outputs.cards)
                .unwrap_or_else(|| crate::result_artifacts::collect(&value)),
            value,
            executed,
        })
    }
}
