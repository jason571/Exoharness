// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    Catalog, DispatchResult, Result, ToolError, ToolRuntime,
    workspace::{task_error, workspace_receipt},
};
use exoharness_model_runtime::CancellationToken;
use exoharness_sandbox::{Limits, RunSpec, Runtime, TaskEdit};
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    runtime: Runtime,
    entrypoint: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
    skill_id: Option<String>,
    workspace_id: Option<String>,
    expected_revision: Option<u64>,
    timeout_seconds: Option<u64>,
    #[serde(default)]
    arguments: Vec<String>,
    output_files: Vec<String>,
}
pub fn safe_output(root: &Path, relative: &str) -> Result<fs::File> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains(['\\', ':'])
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(ToolError::Input("输出文件必须位于本次沙盒工作区".into()));
    }
    let mut current = root.to_path_buf();
    for part in path.components() {
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ToolError::MissingOutput(relative.into())
            } else {
                ToolError::Storage
            }
        })?;
        if meta.file_type().is_symlink() {
            return Err(ToolError::Input("不允许交付符号链接".into()));
        }
    }
    let file = fs::File::open(current).map_err(|_| ToolError::Storage)?;
    if !file.metadata().map_err(|_| ToolError::Storage)?.is_file() {
        return Err(ToolError::Input("输出不是文件".into()));
    }
    Ok(file)
}
impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_sandbox(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        invocation: &str,
        input: &Value,
        cancel: CancellationToken,
    ) -> Result<DispatchResult> {
        let input: Input = serde_json::from_value(input.clone())
            .map_err(|_| ToolError::Input("沙盒参数格式不正确".into()))?;
        let sandbox = self
            .sandbox
            .as_ref()
            .ok_or_else(|| ToolError::Input("隔离执行环境尚未就绪；不会在宿主机执行".into()))?;
        if !input.runtime.valid_entrypoint(&input.entrypoint) {
            return Err(ToolError::Input("运行类型与入口扩展名不符；未执行".into()));
        }
        let store = sandbox.task_store().map_err(task_error)?;
        let mut snapshot = match &input.workspace_id {
            Some(id) => {
                if input.expected_revision.is_none()
                    || !input.files.is_empty()
                    || input.skill_id.is_some()
                {
                    return Err(ToolError::Input("复用工作区需要 expected_revision；修改请用 workspace_edit，不能隐式覆盖文件或更换技能包".into()));
                }
                let saved = store.inspect(owner, session, id).map_err(task_error)?;
                if Some(saved.revision) != input.expected_revision {
                    return Err(ToolError::Input("工作区版本已变化；未执行".into()));
                }
                saved
            }
            None => {
                if input.expected_revision.is_some() || input.files.is_empty() {
                    return Err(ToolError::Input(
                        "新执行需要 files；已有源码请指定 workspace_id 和 expected_revision".into(),
                    ));
                }
                let skill = self
                    .task_skill(catalog, owner, input.skill_id.as_deref())
                    .await?;
                let created = store
                    .create(owner, session, invocation, skill)
                    .map_err(task_error)?;
                store
                    .edit(
                        owner,
                        session,
                        &created.workspace_id,
                        created.revision,
                        &format!("{invocation}:source"),
                        TaskEdit {
                            files: input.files,
                            ..Default::default()
                        },
                    )
                    .map_err(task_error)?
            }
        };
        let mut read_only_package = None;
        let mut package_files = None;
        if let Some(id) = &snapshot.skill_id {
            let record = self.current(catalog, owner, id).await?;
            if Some(record.revision) != snapshot.skill_revision {
                return Err(ToolError::Authorization);
            }
            let skills = self.skills.clone();
            let access = RegistryAccess::User(owner.into());
            let id = id.clone();
            let package = tokio::task::spawn_blocking(move || {
                skills.package_snapshot(&access, &id, record.revision)
            })
            .await
            .map_err(|_| ToolError::Storage)?
            .map_err(|_| ToolError::Authorization)?;
            read_only_package = Some(
                sandbox
                    .create_workspace(&format!("{invocation}:package"), package.files.clone())
                    .map_err(task_error)?,
            );
            package_files = Some(package.files);
        }
        // A valid extension is not evidence that the source exists. Reject
        // before leasing or launching, with the owned snapshot needed to stage
        // the missing file; do not report an opaque path/security failure.
        let authored = snapshot
            .files
            .iter()
            .any(|file| file.path == input.entrypoint);
        let packaged = input.entrypoint.strip_prefix("skill/").is_some_and(|path| {
            package_files
                .as_ref()
                .is_some_and(|files| files.contains_key(path))
        });
        if !authored && !packaged {
            return Err(ToolError::CorrectableInput(format!(
                "入口文件 {} 尚未保存，未执行代码。保留现有文件；用 workspace_edit 在 workspace_id={}、expected_revision={} 保存该入口，或选择已经保存的入口，再调用 sandbox_run（复用工作区时省略 files）。",
                input.entrypoint, snapshot.workspace_id, snapshot.revision
            )));
        }
        let execution = store
            .begin(
                owner,
                session,
                &snapshot.workspace_id,
                snapshot.revision,
                invocation,
            )
            .map_err(task_error)?;
        if let Some(files) = package_files {
            let staged = (|| -> Result<()> {
                for (path, bytes) in files {
                    let destination = execution.workspace.path().join("skill").join(path);
                    let parent = destination.parent().ok_or(ToolError::Storage)?;
                    fs::create_dir_all(parent).map_err(|_| ToolError::Storage)?;
                    fs::write(destination, bytes).map_err(|_| ToolError::Storage)?;
                }
                Ok(())
            })();
            if let Err(error) = staged {
                execution.reject().map_err(task_error)?;
                return Err(error);
            }
        }
        let limits = Limits {
            timeout: std::time::Duration::from_secs(input.timeout_seconds.unwrap_or(300)),
            ..Default::default()
        };
        let entrypoint = input.entrypoint.clone();
        let runtime = input.runtime;
        let job = match sandbox.start(RunSpec {
            invocation_id: invocation.into(),
            workspace: execution.workspace.clone(),
            read_only_package,
            runtime: input.runtime,
            entrypoint: input.entrypoint,
            arguments: input.arguments,
            limits,
        }) {
            Ok(job) => job,
            Err(
                error @ (exoharness_sandbox::SandboxError::Invalid
                | exoharness_sandbox::SandboxError::RuntimeMismatch
                | exoharness_sandbox::SandboxError::WorkspaceBusy
                | exoharness_sandbox::SandboxError::Unavailable),
            ) => {
                execution.reject().map_err(task_error)?;
                return Err(task_error(error));
            }
            Err(_) => return Err(ToolError::Unknown),
        };
        let receipt = tokio::select! {biased;_=cancel.cancelled()=>return Err(ToolError::Unknown),result=job.finish()=>result.map_err(|_|ToolError::Unknown)?};
        if !receipt.cleanup_confirmed {
            return Err(ToolError::Unknown);
        }
        snapshot = execution.finish(&receipt).map_err(|_| ToolError::Unknown)?;
        let failed = receipt.exit_code != Some(0);
        let mut result = workspace_receipt(snapshot.clone(), true, vec![]);
        result.failed = failed;
        result.value["entrypoint"] = json!(entrypoint);
        result.value["runtime"] = json!(runtime);
        result.value["isError"] = json!(failed);
        result.value["execution"] = json!(receipt);
        if !failed && !input.output_files.is_empty() {
            let view = store
                .snapshot_workspace(
                    owner,
                    session,
                    &snapshot.workspace_id,
                    snapshot.revision,
                    &format!("{invocation}:publication"),
                )
                .map_err(|_| ToolError::Unknown)?;
            match self
                .publish_outputs(owner, session, run, view.path(), input.output_files)
                .await
            {
                Ok(artifacts) => {
                    result.value["artifacts"] = json!(artifacts);
                    result.artifacts = artifacts;
                }
                Err(ToolError::Unknown) => return Err(ToolError::Unknown),
                Err(error @ ToolError::MissingOutput(_)) => {
                    // The code completed and its files were committed. Only
                    // publication is pending; never report code as unexecuted
                    // or retry it to correct the selected output name.
                    result.value["_host_publication_pending"] = json!(true);
                    result.value["publication_error"] = json!(error.to_string());
                    result.value["status"] = json!("publication_pending");
                    result.value["recovery"] = json!(
                        "Code completed. Select the intended final file from this saved inventory and call workspace_publish at the returned workspace_id/revision. Do not run the code again. No final output has been delivered yet."
                    );
                }
                Err(error) => {
                    result.failed = true;
                    result.value["isError"] = json!(true);
                    result.value["publication_error"] = json!(error.to_string());
                    result.value["recovery"] = json!(
                        "Output remains saved. Correct only the output selection or source; use workspace_publish without rerunning successful code."
                    );
                }
            }
        }
        Ok(result)
    }
}
