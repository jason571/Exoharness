use super::ChatState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
pub(super) async fn cancel(
    State(state): State<ChatState>,
    Extension(principal): Extension<Principal>,
    Path(run): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let copy = run.clone();
    let owner = principal.id.clone();
    let record = storage(state.store.clone(), move |s| s.chat_cancel(&owner, &copy)).await?;
    state.tasks.cancel(&run).await;
    Ok(Json(
        json!({"run_id":run,"session_id":record.session_id,"status":"cancelled"}),
    ))
}
pub(super) async fn resume(
    State(state): State<ChatState>,
    Extension(principal): Extension<Principal>,
    Path(run): Path<String>,
) -> Result<Json<Value>, ApiError> {
    storage(state.store, move |s| s.owned_run(&principal.id, &run)).await?;
    Err(ApiError(
        StatusCode::CONFLICT,
        "resume_unavailable",
        "此运行没有可安全继续的执行检查点；不会重复调用模型或工具，请重新提交消息",
    ))
}
