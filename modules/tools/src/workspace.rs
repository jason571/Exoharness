// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Thin tool adapters; scope, file CAS and execution ownership live in Sandbox.
use crate::{Catalog, DispatchResult, Result, ToolError, ToolRuntime};
use exoharness_sandbox::{TaskEdit, TaskSnapshot};
use serde_json::{Value, json};
impl ToolRuntime {
    pub(crate) async fn task_skill(
        &self,
        catalog: &Catalog,
        owner: &str,
        skill: Option<&str>,
    ) -> Result<Option<(String, i64)>> {
        if let Some(id) = skill {
            let record = self.current(catalog, owner, id).await?;
            Ok(Some((id.into(), record.revision)))
        } else {
            Ok(None)
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn workspace_operation(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        invocation: &str,
        name: &str,
        input: &Value,
    ) -> Result<DispatchResult> {
        let sandbox = self
            .sandbox
            .as_ref()
            .ok_or_else(|| ToolError::Input("隔离执行环境尚未就绪".into()))?;
        let store = sandbox.task_store().map_err(task_error)?;
        let owner = owner.to_owned();
        let session = session.to_owned();
        let operation = invocation.to_owned();
        let input = input.clone();
        let name = name.to_owned();
        if name == "workspace_import" {
            let id = input["workspace_id"]
                .as_str()
                .ok_or(ToolError::Authorization)?
                .to_owned();
            let revision = input["expected_revision"]
                .as_u64()
                .ok_or(ToolError::Authorization)?;
            let artifacts = self.artifacts.clone();
            let snapshot = tokio::task::spawn_blocking(move || -> Result<TaskSnapshot> {
                let mut files = std::collections::BTreeMap::new();
                for source in input["inputs"].as_array().ok_or(ToolError::Authorization)? {
                    let path = source["path"].as_str().ok_or(ToolError::Authorization)?;
                    let artifact = source["artifact_id"]
                        .as_str()
                        .ok_or(ToolError::Authorization)?;
                    let (_, file) = artifacts
                        .open_owned(&owner, artifact)
                        .map_err(|_| ToolError::Authorization)?;
                    if files.insert(path.to_owned(), file).is_some() {
                        return Err(ToolError::Input("导入路径不能重复".into()));
                    }
                }
                store
                    .import(&owner, &session, &id, revision, &operation, files)
                    .map_err(task_error)
            })
            .await
            .map_err(|_| ToolError::Storage)??;
            return Ok(workspace_receipt(snapshot, false, vec![]));
        }
        if name == "workspace_job" {
            let id = input["workspace_id"]
                .as_str()
                .ok_or(ToolError::Authorization)?;
            let job = input["invocation_id"]
                .as_str()
                .ok_or(ToolError::Authorization)?;
            let receipt = store.job(&owner, &session, id, job).map_err(task_error)?;
            let requested = if input["action"] == "cancel" && receipt.is_none() {
                sandbox.cancel_invocation(job).map_err(task_error)?
            } else {
                false
            };
            return Ok(DispatchResult {
                value: json!({"workspace_id":id,"invocation_id":job,"execution":receipt,"cancel_requested":requested,"status":if receipt.is_some(){"settled"}else if requested{"cancellation_requested"}else{"outcome_not_confirmed"}}),
                executed: false,
                failed: false,
                artifacts: vec![],
            });
        }
        let skill = if name == "workspace_create" {
            self.task_skill(catalog, &owner, input["skill_id"].as_str())
                .await?
        } else {
            None
        };
        if name == "workspace_publish" {
            let workspace_id = input["workspace_id"]
                .as_str()
                .ok_or(ToolError::Authorization)?
                .to_owned();
            let snapshot = store
                .inspect(&owner, &session, &workspace_id)
                .map_err(task_error)?;
            if let Some(id) = &snapshot.skill_id {
                let record = self.current(catalog, &owner, id).await?;
                if Some(record.revision) != snapshot.skill_revision {
                    return Err(ToolError::Authorization);
                }
            }
            let revision = input["expected_revision"]
                .as_u64()
                .ok_or(ToolError::Authorization)?;
            let view = store
                .snapshot_workspace(
                    &owner,
                    &session,
                    &workspace_id,
                    revision,
                    &format!("{operation}:publication"),
                )
                .map_err(task_error)?;
            let outputs = serde_json::from_value(input["output_files"].clone())
                .map_err(|_| ToolError::Input("output_files 必须为数组".into()))?;
            let artifacts = self
                .publish_outputs(&owner, &session, run, view.path(), outputs)
                .await?;
            let mut receipt = workspace_receipt(snapshot, true, artifacts);
            receipt.value["_host_publication_complete"] = json!(true);
            return Ok(receipt);
        }
        let checkpoint = matches!(name.as_str(), "workspace_create" | "workspace_edit");
        let value = tokio::task::spawn_blocking(move || -> Result<Value> {
            let id = input["workspace_id"].as_str().unwrap_or("");
            match name.as_str() {
                "workspace_list" => {
                    Ok(json!({"workspaces":store.list(&owner,&session).map_err(task_error)?}))
                }
                "workspace_create" => Ok(serde_json::to_value(
                    store
                        .create(&owner, &session, &operation, skill)
                        .map_err(task_error)?,
                )
                .map_err(|_| ToolError::Storage)?),
                "workspace_read" => {
                    if let Some(path) = input["path"].as_str() {
                        store
                            .read(
                                &owner,
                                &session,
                                id,
                                path,
                                input["offset"].as_u64().unwrap_or(0) as usize,
                                input["limit"].as_u64().unwrap_or(8000) as usize,
                                input["query"].as_str(),
                            )
                            .map_err(task_error)
                    } else {
                        Ok(serde_json::to_value(
                            store.inspect(&owner, &session, id).map_err(task_error)?,
                        )
                        .map_err(|_| ToolError::Storage)?)
                    }
                }
                "workspace_edit" => {
                    let mut edit = input.clone();
                    let revision = edit["expected_revision"]
                        .as_u64()
                        .ok_or(ToolError::Authorization)?;
                    edit.as_object_mut()
                        .ok_or(ToolError::Authorization)?
                        .remove("workspace_id");
                    edit.as_object_mut().unwrap().remove("expected_revision");
                    let edit: TaskEdit = serde_json::from_value(edit)
                        .map_err(|_| ToolError::Input("工作文件修改格式不正确".into()))?;
                    Ok(serde_json::to_value(
                        store
                            .edit(&owner, &session, id, revision, &operation, edit)
                            .map_err(task_error)?,
                    )
                    .map_err(|_| ToolError::Storage)?)
                }
                _ => Err(ToolError::Authorization),
            }
        })
        .await
        .map_err(|_| ToolError::Storage)??;
        let mut result = DispatchResult {
            value,
            executed: false,
            failed: false,
            artifacts: vec![],
        };
        if checkpoint {
            result.value["_workspace_checkpoint"] = json!(true);
        }
        Ok(result)
    }
}
pub(crate) fn task_error(error: exoharness_sandbox::SandboxError) -> ToolError {
    match error {
        exoharness_sandbox::SandboxError::OutcomeUnknown
        | exoharness_sandbox::SandboxError::CleanupPending => ToolError::Unknown,
        _ => ToolError::Input(error.to_string()),
    }
}
pub(crate) fn workspace_receipt(
    snapshot: TaskSnapshot,
    executed: bool,
    artifacts: Vec<Value>,
) -> DispatchResult {
    let mut value = serde_json::to_value(snapshot).expect("serializable task receipt");
    value["artifacts"] = json!(artifacts);
    value["_workspace_checkpoint"] = json!(true);
    DispatchResult {
        value,
        executed,
        failed: false,
        artifacts,
    }
}
