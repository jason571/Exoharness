//! Host observes only the background task explicitly declared by this MCP.
//! Polls never enter the model loop or reset its repeated-call protection.
use crate::{Catalog, Result, ToolError, ToolRuntime, validate};
use exoharness_mcp::{Connection, Progress};
use exoharness_model_runtime::CancellationToken;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::watch;

#[cfg(test)]
#[path = "../tests/unit/background.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Action {
    pub tool: String,
    pub arguments: Value,
}
#[derive(Clone, Debug)]
pub(crate) struct Task {
    pub id: String,
    pub status: String,
    pub poll: Action,
    pub cancel: Option<Action>,
    pub interval: Duration,
    pub raw: Value,
}
fn action(value: &Value) -> Result<Action> {
    let tool = value["tool"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        .ok_or(ToolError::Unknown)?;
    if !value["arguments"].is_object() {
        return Err(ToolError::Unknown);
    }
    Ok(Action {
        tool: tool.into(),
        arguments: value["arguments"].clone(),
    })
}
pub(crate) fn declared(value: &Value) -> Result<Option<Task>> {
    // MCP extensions may accompany the result envelope or structured content.
    // Never silently choose between contradictory task ownership declarations.
    let envelope = value.pointer("/_meta/exoharness/background_task");
    let structured = value.pointer("/structuredContent/_meta/exoharness/background_task");
    if let (Some(left), Some(right)) = (envelope, structured)
        && left != right
    {
        return Err(ToolError::Unknown);
    }
    let Some(raw) = envelope.or(structured) else {
        return Ok(None);
    };
    if raw["version"] != 1
        || serde_json::to_vec(raw)
            .map_err(|_| ToolError::Unknown)?
            .len()
            > 65536
    {
        return Err(ToolError::Unknown);
    }
    let id = raw["id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .ok_or(ToolError::Unknown)?;
    let status = raw["status"]
        .as_str()
        .filter(|s| {
            matches!(
                *s,
                "queued" | "working" | "running" | "completed" | "failed" | "cancelled"
            )
        })
        .ok_or(ToolError::Unknown)?;
    Ok(Some(Task {
        id: id.into(),
        status: status.into(),
        poll: action(&raw["poll"])?,
        cancel: raw.get("cancel").map(action).transpose()?,
        interval: Duration::from_millis(
            raw["poll_interval_ms"]
                .as_u64()
                .unwrap_or(2500)
                .clamp(500, 30000),
        ),
        raw: raw.clone(),
    }))
}
fn schema<'a>(tools: &'a [Value], action: &Action) -> Result<&'a Value> {
    let tool = tools
        .iter()
        .find(|t| t["name"] == action.tool)
        .ok_or(ToolError::Unknown)?;
    validate(&tool["inputSchema"], &action.arguments).map_err(|_| ToolError::Unknown)?;
    Ok(&tool["inputSchema"])
}
fn progress(task: &Task, sender: &watch::Sender<Option<Progress>>) {
    let data = &task.raw["progress"];
    sender.send_replace(Some(Progress {
        token: json!(task.id),
        progress: data["progress"].as_f64().unwrap_or(0.0),
        total: data["total"].as_f64(),
        message: data["message"].as_str().map(str::to_owned),
    }));
}
async fn cancel_once(connection: &Connection, task: &Task, tools: &[Value]) -> Option<Value> {
    if let Some(action) = &task.cancel
        && schema(tools, action).is_ok()
    {
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            connection.call(
                &action.tool,
                action.arguments.clone(),
                |_| {},
                std::future::pending::<()>(),
            ),
        )
        .await;
        if let Ok(Ok(value)) = result {
            return confirmed_terminal(value, &task.id);
        }
    }
    None
}

// A cancellation acknowledgement alone does not free execution capacity.
// Trust only this MCP's explicit terminal declaration for the same owned job.
fn confirmed_terminal(mut value: Value, id: &str) -> Option<Value> {
    let task = declared(&value).ok()??;
    if task.id != id || !matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return None;
    }
    if task.status != "completed" {
        value["isError"] = json!(true);
    }
    Some(value)
}
impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn observe_mcp(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        invocation: &str,
        capability: &str,
        connection: &Connection,
        tools: &[Value],
        mut value: Value,
        cancel: &CancellationToken,
        sender: &watch::Sender<Option<Progress>>,
    ) -> Result<Value> {
        let Some(first) = declared(&value)? else {
            return Ok(value);
        };
        if matches!(first.status.as_str(), "completed" | "failed" | "cancelled") {
            if first.status != "completed" {
                value["isError"] = json!(true);
            }
            return Ok(value);
        }
        schema(tools, &first.poll)?;
        if let Some(action) = &first.cancel {
            schema(tools, action)?;
        }
        // Commit task ownership and recovery controls before waiting. This is
        // execution evidence, not a model-visible poll or a fake final receipt.
        let store = self.execution.clone();
        let (owner_copy, session_copy, run_copy) =
            (owner.to_owned(), session.to_owned(), run.to_owned());
        let evidence = json!({"type":"mcp_background_task","tool_call_id":invocation,"capability_id":capability,"task":first.raw});
        tokio::task::spawn_blocking(move || {
            store.chat_event(&owner_copy, &session_copy, &run_copy, evidence)
        })
        .await
        .map_err(|_| ToolError::Unknown)?
        .map_err(|_| ToolError::Unknown)?;
        let mut current = first.clone();
        loop {
            progress(&current, sender);
            if matches!(
                current.status.as_str(),
                "completed" | "failed" | "cancelled"
            ) {
                if current.status != "completed" {
                    value["isError"] = json!(true);
                }
                return Ok(value);
            }
            tokio::select! {biased;
                ()=cancel.cancelled()=>{
                    return cancel_once(connection,&first,tools).await.ok_or(ToolError::Unknown);
                }
                ()=tokio::time::sleep(current.interval)=>{}
            }
            // The same registered capability must remain enabled and authorized.
            self.current(catalog, owner, capability)
                .await
                .map_err(|_| ToolError::Unknown)?;
            value = match connection
                .call(
                    &first.poll.tool,
                    first.poll.arguments.clone(),
                    |p| {
                        sender.send_replace(Some(p));
                    },
                    cancel.cancelled(),
                )
                .await
            {
                Ok(value) => value,
                Err(_) => {
                    if cancel.is_cancelled() {
                        return cancel_once(connection, &first, tools)
                            .await
                            .ok_or(ToolError::Unknown);
                    }
                    return Err(ToolError::Unknown);
                }
            };
            if value["isError"] == true {
                return Ok(value);
            }
            current = declared(&value)?.ok_or(ToolError::Unknown)?;
            if current.id != first.id || current.poll != first.poll {
                return Err(ToolError::Unknown);
            }
        }
    }
}
