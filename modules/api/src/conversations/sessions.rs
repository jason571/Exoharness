use super::ConversationState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize, Default)]
pub(super) struct History {
    pub turn_limit: Option<usize>,
    pub before_ordinal: Option<i64>,
}
#[derive(Deserialize, Default)]
pub(super) struct Search {
    q: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewSession {
    title: Option<String>,
    actor: Option<String>,
}

pub(super) async fn list(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<Search>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        storage(state.store, move |store| store
            .conversations(&principal.id, query.q.as_deref().unwrap_or("")))
        .await?
    )))
}
pub(super) async fn create(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<NewSession>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    // Browser's actor label is not an identity or permission source.
    let _ = body.actor;
    let automatic_title = body.title.is_none();
    let title = body.title.unwrap_or_else(|| "新对话".into());
    let assistants = state.assistants.clone();
    tokio::task::spawn_blocking(move || assistants.refresh())
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "assistant_worker_failed",
                "助理配置读取中断",
            )
        })?
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "assistant_unavailable",
                "助理配置暂时不可用",
            )
        })?;
    let mut profile = state.assistants.default_public();
    if !state.management
        && let Some(identity) = state.identity
    {
        let owner = principal.id.clone();
        if let Some(id) =
            crate::identity::work(identity, move |store| store.user_agent_profile_id(&owner))
                .await?
        {
            let assigned = state.assistants.public_profile(&id);
            if !assigned.is_null() {
                profile = assigned;
            }
        }
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_failed",
            "无法创建安全标识",
        )
    })?;
    let id = format!(
        "session-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    Ok((
        StatusCode::CREATED,
        Json(
            storage(state.store, move |store| {
                store.new_conversation_with_title(
                    &principal.id,
                    &id,
                    &title,
                    &profile,
                    automatic_title,
                )
            })
            .await?,
        ),
    ))
}
pub(super) async fn detail(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    Query(query): Query<History>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        storage(state.store, move |store| {
            store.conversation_detail(
                &principal.id,
                &session,
                query.before_ordinal,
                query.turn_limit.unwrap_or(30),
            )
        })
        .await?,
    ))
}
pub(super) async fn update(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        storage(state.store, move |store| {
            store.update_conversation(&principal.id, &session, &body)
        })
        .await?,
    ))
}
pub(super) async fn delete(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
) -> Result<StatusCode, ApiError> {
    storage(state.store, move |store| {
        store.delete_conversation(&principal.id, &session)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
