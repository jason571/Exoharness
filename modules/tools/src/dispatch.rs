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
                if catalog.read.contains(&(id.into(), path.into())) {
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
                        ToolError::Input("技能包中没有此相对路径；未读取宿主机文件".into())
                    })?;
                    if bytes.len() > 256 * 1024 {
                        return Err(ToolError::Input(
                            "文件过大，请先选择较小的参考文件；未将二进制或大文件发送给模型".into(),
                        ));
                    }
                    let text = std::str::from_utf8(bytes).map_err(|_| {
                        ToolError::Input("此文件不是UTF-8说明文件；不返回编码数据".into())
                    })?;
                    catalog.read.insert((id.into(), path.into()));
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
                    let mut value = json!({"id":id,"revision":record.revision,"path":path,"content":text,"files":package.files.keys().collect::<Vec<_>>(),"execution_guidance":catalog.execution_guidance()});
                    if let Some(context) = context {
                        if context["auto_activate_entry"] == true {
                            // A verified immutable package can load uniquely
                            // resolved authorized schemas, never grant or run tools.
                            let names: Vec<String> =
                                serde_json::from_value(context["entry_tools"].clone())
                                    .map_err(|_| ToolError::Storage)?;
                            catalog.active.extend(names);
                        }
                        value["_host_skill_context"] = context;
                    }
                    value
                }
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
                        return Err(ToolError::Input(
                            "MCP Schema已变化，请重新验证服务后开启新运行；未执行".into(),
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
                    let value = self
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
        if let Some(cards) = &delivered
            && !cards.is_empty()
        {
            // Preserve the exact MCP fields; host delivery receipts are
            // separate metadata, not changes to the service result.
            value["_host_delivered_artifacts"] = json!(cards);
        }
        Ok(DispatchResult {
            failed: value["isError"] == true,
            artifacts: delivered.unwrap_or_else(|| crate::result_artifacts::collect(&value)),
            value,
            executed,
        })
    }
}
