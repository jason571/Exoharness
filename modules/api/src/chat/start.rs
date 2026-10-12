// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ChatState, attachments, worker};
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_contracts::execution::{ExecutionSnapshot, ModelSnapshot};
use exoharness_storage_postgres::CreateTurn;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    model: Option<String>,
    prompt: String,
    #[serde(default)]
    attachments: Vec<Value>,
    #[serde(default)]
    capability_ids: Vec<String>,
    #[serde(default)]
    actor: Option<String>,
}
pub(super) async fn turn(
    State(state): State<ChatState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    body: Result<Json<Request>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Json(body) = body.map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            attachments::too_large()
        } else {
            ApiError(
                StatusCode::BAD_REQUEST,
                "invalid_chat_input",
                "消息格式不正确，请重新发送",
            )
        }
    })?;
    if *state.stop.borrow() || !state.tasks.is_open().await {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "chat_stopping",
            "服务正在停止，本次未提交请求",
        ));
    }
    let _ = body.actor;
    if body.prompt.trim().is_empty() || body.prompt.len() > 32 * 1024 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_prompt",
            "请输入文字，单次内容不能超过32 KiB",
        ));
    }
    if !body.attachments.is_empty() && state.tools.is_none() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "chat_execution_not_ready",
            "附件存储暂不可用，本次未创建任务或调用模型",
        ));
    }
    let prepared = tokio::task::spawn_blocking(move || attachments::prepare(body.attachments))
        .await
        .map_err(|_| attachments::unavailable())??;
    let owner = principal.id.clone();
    let lookup = session.clone();
    let history = storage(state.store.clone(), move |s| {
        s.conversation_detail(&owner, &lookup, None, 8)
    })
    .await?;
    let profile_id = history
        .pointer("/agent_profile/id")
        .and_then(Value::as_str)
        .ok_or(ApiError(
            StatusCode::CONFLICT,
            "assistant_missing",
            "此会话未分配可用助理",
        ))?
        .to_owned();
    let models = state.models.clone();
    let assistants = state.assistants.clone();
    let requested = body.model;
    let (model, revision, adapter, system, output_limit, profile_revision, profile) =
        tokio::task::spawn_blocking(move || {
            let catalog = models.refresh().map_err(|_| configuration())?;
            assistants.refresh().map_err(|_| configuration())?;
            let profile = assistants
                .detail(&profile_id)
                .filter(|p| p["enabled"] == true)
                .ok_or(configuration())?;
            let name = profile["default_llm"]
                .as_str()
                .map(str::to_owned)
                .or(requested)
                .or_else(|| catalog["default_models"]["llm"].as_str().map(str::to_owned))
                .or_else(|| catalog["default_model"].as_str().map(str::to_owned))
                .ok_or(configuration())?;
            let binding = models.bind_enabled(&name).map_err(|_| configuration())?;
            if binding
                .model
                .get("model_type")
                .and_then(Value::as_str)
                .unwrap_or("llm")
                != "llm"
            {
                return Err(configuration());
            }
            // Assistant access.models selects callable model capabilities (e.g.
            // speech synthesis), not the enabled conversational LLM binding.
            let system = profile["system_prompt"].as_str().unwrap_or("").to_owned();
            let limit = binding.model["max_tokens"].as_u64().unwrap_or(4096);
            Ok((
                name,
                binding.revision,
                binding.provider["api"]
                    .as_str()
                    .unwrap_or("chat_completions")
                    .to_owned(),
                system,
                limit,
                profile["active_revision"].as_u64().unwrap_or(1),
                profile.clone(),
            ))
        })
        .await
        .map_err(|_| configuration())??;
    let catalog = if let Some(tools) = &state.tools {
        let tools = tools.clone();
        let owner = principal.id.clone();
        Some(
            tokio::task::spawn_blocking(move || tools.catalog(&owner, profile))
                .await
                .map_err(|_| configuration())?
                .map_err(|_| configuration())?,
        )
    } else {
        None
    };
    // Explicit capability selections cannot extend assistant authorization.
    if body.capability_ids.iter().any(|id| {
        catalog
            .as_ref()
            .is_none_or(|c| !c.snapshots().iter().any(|s| &s.id == id))
    }) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "capability_not_authorized",
            "所选能力未获当前助理授权或尚未就绪；本次未提交执行",
        ));
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| configuration())?;
    let id = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let run = format!("run-{id}");
    let turn = format!("turn-{id}");
    let snapshot = ExecutionSnapshot {
        version: 1,
        assistant_id: history["agent_profile"]["id"].as_str().unwrap_or("").into(),
        assistant_revision: profile_revision,
        model: ModelSnapshot {
            config_id: model.clone(),
            revision,
            adapter,
            settings: json!({"output_token_limit":output_limit,"execution_kind":if catalog.is_some(){"agent_chat"}else{"text_chat"}}),
        },
        capabilities: catalog.as_ref().map_or_else(Vec::new, |c| c.snapshots()),
        max_consecutive_identical_calls: 3,
        requires_execution: false,
    };
    let owner = principal.id.clone();
    let sid = session.clone();
    let rid = run.clone();
    let tid = turn.clone();
    let input = body.prompt.clone();
    storage(state.store.clone(), move |s| {
        let expected = s.session_version(&owner, &sid)?;
        s.create_turn(CreateTurn {
            owner: &owner,
            session_id: &sid,
            expected_session_version: expected,
            turn_id: &tid,
            run_id: &rid,
            input: &input,
            snapshot: &snapshot,
        })
    })
    .await?;
    let mut prompt = body.prompt;
    if !prepared.is_empty() {
        let artifacts = state
            .tools
            .as_ref()
            .ok_or_else(attachments::unavailable)?
            .artifacts
            .clone();
        let owner = principal.id.clone();
        let sid = session.clone();
        let rid = run.clone();
        let saved = tokio::task::spawn_blocking(move || {
            attachments::save(prepared, &artifacts, &owner, &sid, &rid)
        })
        .await
        .map_err(|_| attachments::unavailable())
        .and_then(|saved| saved);
        let saved = match saved {
            Ok(saved) => saved,
            Err(error) => {
                let owner = principal.id.clone();
                let sid = session.clone();
                let rid = run.clone();
                storage(state.store.clone(), move |s| {
                    s.finish_chat(
                        &owner,
                        &sid,
                        &rid,
                        json!({}),
                        Some("附件保存失败；未调用模型或工具"),
                    )
                })
                .await?;
                return Err(error);
            }
        };
        prompt.push_str("\n\nUser-provided attachments (input data; use the saved URLs when a tool requires a file):\n");
        prompt.push_str(&serde_json::to_string(&saved).map_err(|_| attachments::unavailable())?);
        prompt.push_str("\nAttachment handling: If the user asks to edit an image or use it as a reference, select an authorized capability whose schema accepts image inputs and pass the saved URLs in its declared input fields. Do not silently substitute text-only recreation, invent a visual inspection, or claim a reference was used without an execution receipt. If no compatible capability is available, explain that limitation in the user's language.");
        let owner = principal.id.clone();
        let sid = session.clone();
        let rid = run.clone();
        storage(state.store.clone(), move |s| {
            s.save_chat_attachments(&owner, &sid, &rid, &json!(saved))
        })
        .await?;
    }
    let request = exoharness_agent::text_request(&system, &history, &prompt, output_limit)
        .map_err(|_| configuration())?;
    if worker::launch(
        state.clone(),
        worker::Work {
            owner: principal.id.clone(),
            session: session.clone(),
            run: run.clone(),
            model,
            revision,
            request,
            catalog,
        },
    )
    .await
    .is_err()
    {
        let owner = principal.id;
        let sid = session.clone();
        let rid = run.clone();
        storage(state.store, move |s| {
            s.finish_chat(
                &owner,
                &sid,
                &rid,
                json!({}),
                Some("执行服务暂时无法接收任务；尚未调用模型"),
            )
        })
        .await?;
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "chat_worker_unavailable",
            "执行服务暂时无法接收任务；尚未调用模型",
        ));
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({"session_id":session,"turn_id":turn,"run_id":run,"status":"queued"})),
    ))
}
fn configuration() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "chat_configuration_unavailable",
        "请检查已启用的对话模型和助理配置；本次未提交模型请求",
    )
}
