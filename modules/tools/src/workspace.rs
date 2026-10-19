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
                        .map_err(import_artifact_error)?;
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
        let continuation = matches!(name.as_str(), "workspace_list" | "workspace_read");
        let context_owner = owner.clone();
        let context_session = session.clone();
        let mut value = tokio::task::spawn_blocking(move || -> Result<Value> {
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
                            .map_err(edit_error)?,
                    )
                    .map_err(|_| ToolError::Storage)?)
                }
                _ => Err(ToolError::Authorization),
            }
        })
        .await
        .map_err(|_| ToolError::Storage)??;
        if continuation {
            self.workspace_continuation(catalog, &context_owner, &context_session, &mut value)
                .await?;
        }
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

impl ToolRuntime {
    async fn workspace_continuation(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        value: &mut Value,
    ) -> Result<()> {
        if catalog.binding("sandbox_run").is_none() {
            return Ok(());
        }
        // A list does not select among multiple projects. Enrich only the one
        // unambiguous workspace, or the workspace explicitly chosen for a read.
        let target = if let Some(items) = value["workspaces"].as_array() {
            if items.len() != 1 {
                return Ok(());
            }
            &items[0]
        } else {
            &*value
        };
        let Some(id) = target["workspace_id"].as_str().map(str::to_owned) else {
            return Ok(());
        };
        let Some(revision) = target["revision"].as_u64() else {
            return Ok(());
        };
        let store = self.execution.clone();
        let (o, s, w) = (owner.to_owned(), session.to_owned(), id.clone());
        let Some(mut recipe) =
            tokio::task::spawn_blocking(move || store.workspace_execution_recipe(&o, &s, &w))
                .await
                .map_err(|_| ToolError::Storage)?
                .map_err(|_| ToolError::Storage)?
        else {
            return Ok(());
        };
        let sandbox = self.sandbox.as_ref().ok_or(ToolError::Storage)?;
        let task_store = sandbox.task_store().map_err(task_error)?;
        let (o, s, w) = (owner.to_owned(), session.to_owned(), id.clone());
        let snapshot = tokio::task::spawn_blocking(move || task_store.inspect(&o, &s, &w))
            .await
            .map_err(|_| ToolError::Storage)?
            .map_err(task_error)?;
        if snapshot.revision != revision {
            return Ok(());
        }
        let Some(entry) = recipe["arguments"]["entrypoint"].as_str() else {
            return Ok(());
        };
        if entry.starts_with("skill/") {
            if recipe["skill_id"].as_str() != snapshot.skill_id.as_deref()
                || recipe["skill_revision"].as_i64() != snapshot.skill_revision
            {
                return Ok(());
            }
        } else if !snapshot.files.iter().any(|file| {
            file.path == entry && recipe["entrypoint_sha256"].as_str() == Some(file.sha256.as_str())
        }) {
            return Ok(());
        }
        if let Some(skill) = recipe["skill_id"].as_str() {
            let Ok(current) = self.current(catalog, owner, skill).await else {
                return Ok(());
            };
            if recipe["skill_revision"].as_i64() != Some(current.revision) {
                return Ok(());
            }
            recipe["instructions"] =
                json!({"tool":"read_skill","arguments":{"id":skill,"path":"SKILL.md"}});
        }
        recipe["arguments"]["workspace_id"] = json!(id);
        recipe["arguments"]["expected_revision"] = json!(revision);
        recipe["guidance"] = json!(
            "This saved program already delivered files. For a follow-up, inspect and edit only the task inputs needed for the requested change, then reuse this entrypoint and arguments with the NEW revision returned by the edit. Read the bound Skill instructions when needed. Do not probe the prepared environment, invent a replacement entrypoint or rewrite a passing program merely because this is a new Turn. This receipt does not grant execution permission or replay any action."
        );
        value["execution_recipe"] = recipe;
        Ok(())
    }
}

fn import_artifact_error(error: exoharness_artifacts::ArtifactError) -> ToolError {
    match error {
        exoharness_artifacts::ArtifactError::Store(
            exoharness_storage_postgres::StoreError::NotFound,
        ) => ToolError::CorrectableInput(
            "No files were changed. The artifact_id does not identify a saved file available to this user. Use an owned artifact ID from a saved-file receipt; an external task ID is not an artifact ID. Use artifact_id from the host _host_saved_artifacts or _host_delivered_artifacts receipt. If the completed result only has a URL, discover an authorized service accepting that URL; querying the job does not necessarily persist a file. The sandbox cannot download it. Reuse the existing task result instead of submitting another generation."
                .into(),
        ),
        _ => ToolError::Storage,
    }
}
pub(crate) fn task_error(error: exoharness_sandbox::SandboxError) -> ToolError {
    match error {
        exoharness_sandbox::SandboxError::OutcomeUnknown
        | exoharness_sandbox::SandboxError::CleanupPending => ToolError::Unknown,
        _ => ToolError::Input(error.to_string()),
    }
}
/// Only pure, atomic edits can attest that these errors changed no files.
/// Execution/storage/authorization and busy/unknown jobs keep their old path.
pub(crate) fn edit_error(error: exoharness_sandbox::SandboxError) -> ToolError {
    match error {
        exoharness_sandbox::SandboxError::AmbiguousPatch
        | exoharness_sandbox::SandboxError::StaleRevision
        | exoharness_sandbox::SandboxError::PatchRejected(_) => {
            ToolError::CorrectableInput(format!(
                "{error}. No files were changed. Read only the relevant current file range and workspace revision, then correct the exact patch to match once without overlaps. Preserve all unrelated source and do not repeat an unchanged rejected edit."
            ))
        }
        other => task_error(other),
    }
}
#[cfg(test)]
#[path = "../tests/unit/workspace_errors.rs"]
mod tests;
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
