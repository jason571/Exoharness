//! Engineering adapter over the existing authorized workspace and OCI service.
use crate::{
    Catalog, DispatchResult, Result, ToolError, ToolRuntime,
    workspace::{task_error, workspace_receipt},
};
use exoharness_model_runtime::CancellationToken;
use serde_json::{Value, json};
impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn coding_operation(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        invocation: &str,
        name: &str,
        input: &Value,
        cancel: CancellationToken,
    ) -> Result<DispatchResult> {
        if name == "coding_check" {
            let label = input["name"].clone();
            let mut args = input.clone();
            args.as_object_mut()
                .ok_or(ToolError::Authorization)?
                .remove("name");
            args["output_files"] = json!([]);
            let mut result = self
                .run_sandbox(catalog, owner, session, run, invocation, &args, cancel)
                .await?;
            let status = result.value["execution"]["status"].as_str().unwrap_or("");
            // Only a confirmed ordinary process exit is a repairable check.
            // Resource termination/cancellation is not a failed assertion.
            if status != "completed" && status != "failed" {
                return Ok(result);
            }
            result.value["check"] = json!({"name":label,"status":if result.failed {"failed"} else {"passed"},"final_delivery":false});
            result.value["isError"] = json!(false);
            result.value["recovery"] = json!(
                "Inspect exact diagnostics; preserve passing work, patch only relevant source and rerun this check at the returned NEW revision. Do not repeat unchanged checks or install missing dependencies without administrator preparation."
            );
            result.failed = false;
            result.executed = false; // checks cannot stand in for a delivered artifact
            return Ok(result);
        }
        let sandbox = self
            .sandbox
            .as_ref()
            .ok_or_else(|| ToolError::Input("隔离执行环境尚未就绪".into()))?;
        let store = sandbox.task_store().map_err(task_error)?;
        let id = input["workspace_id"]
            .as_str()
            .ok_or(ToolError::Authorization)?
            .to_owned();
        if name == "coding_publish" {
            let revision = input["expected_revision"]
                .as_u64()
                .ok_or(ToolError::Authorization)?;
            let snapshot = store.inspect(owner, session, &id).map_err(task_error)?;
            if let Some(skill) = &snapshot.skill_id
                && Some(self.current(catalog, owner, skill).await?.revision)
                    != snapshot.skill_revision
            {
                return Err(ToolError::Authorization);
            }
            let view = store
                .snapshot_workspace(
                    owner,
                    session,
                    &id,
                    revision,
                    &format!("{invocation}:publication"),
                )
                .map_err(task_error)?;
            let paths: Vec<String> = serde_json::from_value(input["files"].clone())
                .map_err(|_| ToolError::Authorization)?;
            let entrypoint = input["entrypoint"].as_str().map(str::to_owned);
            let mut files = std::collections::BTreeMap::new();
            for path in paths {
                exoharness_coding::paths::WorkspacePath::parse(&path)
                    .map_err(|_| ToolError::Input("项目文件必须是工作区相对路径".into()))?;
                if path.split('/').next() == Some("skill") {
                    return Err(ToolError::Authorization);
                }
                files.insert(
                    path.clone(),
                    crate::sandbox::safe_output(view.path(), &path)?,
                );
            }
            let service = self.artifacts.clone();
            let (owner, session, run) = (owner.to_owned(), session.to_owned(), run.to_owned());
            let card = tokio::task::spawn_blocking(move || {
                service.publish_project(&owner, &session, &run, entrypoint.as_deref(), files)
            })
            .await
            .map_err(|_| ToolError::Unknown)?
            .map_err(|_| ToolError::Input("项目发布失败；已有源码保留，未执行代码".into()))?;
            return Ok(workspace_receipt(snapshot, true, vec![card]));
        }
        let checkpoint = name == "coding_apply_patch";
        let (owner, session, invocation, name, input) = (
            owner.to_owned(),
            session.to_owned(),
            invocation.to_owned(),
            name.to_owned(),
            input.clone(),
        );
        let value = tokio::task::spawn_blocking(move || -> Result<Value> {
            match name.as_str() {
                "coding_search" => store
                    .search(
                        &owner,
                        &session,
                        &id,
                        input["query"].as_str().unwrap_or(""),
                        input["prefix"].as_str().unwrap_or(""),
                        input["file_offset"].as_u64().unwrap_or(0) as usize,
                        input["line_offset"].as_u64().unwrap_or(0) as usize,
                        input["limit"].as_u64().unwrap_or(40) as usize,
                    )
                    .map_err(task_error),
                "coding_apply_patch" => serde_json::to_value(
                    store
                        .apply_patch(
                            &owner,
                            &session,
                            &id,
                            input["expected_revision"]
                                .as_u64()
                                .ok_or(ToolError::Authorization)?,
                            &invocation,
                            input["patch"].as_str().ok_or(ToolError::Authorization)?,
                        )
                        .map_err(|error| match error {
                            exoharness_sandbox::SandboxError::PatchRejected(message) => {
                                ToolError::CorrectableInput(message)
                            }
                            other => task_error(other),
                        })?,
                )
                .map_err(|_| ToolError::Storage),
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
