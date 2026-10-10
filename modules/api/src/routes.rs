use crate::{
    ApiState, Surface,
    auth::{self, Principal, Role},
    errors::{ApiError, storage},
    events,
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    middleware,
    response::Html,
    routing::{get, post},
};
use exoharness_contracts::execution::{
    ExecutionSnapshot, ModelSnapshot, RecoveryAction, RecoveryDecision, RunState,
};
use exoharness_storage_postgres::CreateTurn;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn router(state: ApiState) -> Router {
    let protected = Router::new()
        .route("/v1/application-info", get(crate::application::info))
        .route("/v1/auth/session", get(crate::console::session))
        .route("/v1/auth/logout", post(crate::console::logout))
        .route("/v1/local/status", get(status))
        .route("/v1/local/sessions", get(sessions).post(create_session))
        .route(
            "/v1/local/sessions/{session}/runs",
            get(runs).post(create_run),
        )
        .route(
            "/v1/local/sessions/{session}/runs/{run}",
            get(run).patch(transition),
        )
        .route(
            "/v1/local/sessions/{session}/runs/{run}/events",
            get(events::page),
        )
        .route(
            "/v1/local/sessions/{session}/runs/{run}/events/stream",
            get(events::stream),
        )
        .route("/v1/chat", post(unavailable))
        .route("/v1/execute/python", post(sandbox_unavailable))
        .route("/v1/execute/shell", post(sandbox_unavailable))
        .route("/v1/execute/skill", post(sandbox_unavailable))
        .layer(middleware::from_fn_with_state(state.clone(), auth::gate));
    Router::new().merge(protected).merge(crate::console::routes())
        .route("/health",get(health))
        .route("/local-validation",get(index))
        .route("/local-runtime.js",get(||async{([("content-type","text/javascript; charset=utf-8")],include_str!("../../../web/local-runtime.js"))}))
        .route("/local-runtime.css",get(||async{([("content-type","text/css; charset=utf-8")],include_str!("../../../web/local-runtime.css"))}))
        .fallback(||async{(StatusCode::NOT_FOUND,Json(json!({"error":{"code":"not_implemented","message":"此功能尚未接入新框架"},"resumable":false})))})
        .layer(DefaultBodyLimit::max(64*1024))
        .with_state(state)
}

async fn index(State(state): State<ApiState>) -> Html<String> {
    Html(include_str!("../../../web/local-runtime.html").replace(
        "%SURFACE%",
        match state.surface {
            Surface::Management => "管理端",
            Surface::Portal => "用户端",
        },
    ))
}
async fn health(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    storage(state.execution.0.clone(), |store| store.health()).await?;
    Ok(Json(
        json!({"service":"exoharness","status":"ok","mode":"infrastructure_validation","surface":match state.surface {Surface::Management=>"management",Surface::Portal=>"portal"},"product_ready":false}),
    ))
}
async fn status(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Json<Value> {
    Json(
        json!({"principal":principal,"surface":match state.surface {Surface::Management=>"management",Surface::Portal=>"portal"},"mode":"infrastructure_validation","product_ready":false,"ready":["postgres_execution","authenticated_http","events_replay","run_state_machine"],"unavailable":["agent","models","mcp","skills_execution","chat"]}),
    )
}
async fn unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "agent_not_ready",
        "模型与工具派发尚未接通，本次未创建或提交执行任务",
    )
}

async fn sandbox_unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "sandbox_not_ready",
        "沙盒执行尚未接通，未执行任何代码；不允许回退到宿主机",
    )
}

#[derive(Deserialize, Default)]
pub(crate) struct ReadQuery {
    pub owner: Option<String>,
    pub limit: Option<usize>,
}
pub(crate) fn owner(
    state: &ApiState,
    principal: &Principal,
    requested: Option<String>,
) -> Result<String, ApiError> {
    match requested {
        Some(owner)
            if owner != principal.id
                && !(state.surface == Surface::Management && principal.role == Role::Admin) =>
        {
            Err(ApiError(
                StatusCode::FORBIDDEN,
                "owner_scope",
                "不能访问其他用户的记录",
            ))
        }
        Some(owner) => Ok(owner),
        None => Ok(principal.id.clone()),
    }
}
async fn sessions(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, ApiError> {
    let scope = if state.surface == Surface::Management && principal.role == Role::Admin {
        None
    } else {
        Some(principal.id)
    };
    let records = storage(state.execution.0.clone(), move |store| {
        store.sessions(scope.as_deref(), 100)
    })
    .await?;
    Ok(Json(json!({"sessions":records})))
}
fn random_id() -> Result<String, ApiError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_failed",
            "无法创建安全标识",
        )
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
async fn create_session(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let id = format!("session-{}", random_id()?);
    let saved = id.clone();
    storage(state.execution.0.clone(), move |store| {
        store.create_session(&principal.id, &saved)
    })
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"session_id":id,"version":0})),
    ))
}
async fn runs(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    axum::extract::Query(query): axum::extract::Query<ReadQuery>,
) -> Result<Json<Value>, ApiError> {
    let owner = owner(&state, &principal, query.owner)?;
    let records = storage(state.execution.0.clone(), move |store| {
        store.session_runs(&owner, &session, query.limit.unwrap_or(100))
    })
    .await?;
    Ok(Json(json!({"runs":records})))
}
async fn run(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path((session, run)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<ReadQuery>,
) -> Result<Json<Value>, ApiError> {
    let owner = owner(&state, &principal, query.owner)?;
    let record = storage(state.execution.0.clone(), move |store| {
        store.run(&owner, &session, &run)
    })
    .await?;
    Ok(Json(json!({"run":record})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewRun {
    request_id: String,
    expected_session_version: u64,
    input: String,
}
async fn create_run(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    body: Result<Json<NewRun>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Json(body) = body.map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "请输入原生 JSON，并使用声明字段",
        )
    })?;
    if body.request_id.is_empty() || body.request_id.len() > 128 || body.input.len() > 32 * 1024 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "请求标识或输入长度不合法",
        ));
    }
    let key = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(&principal.id, &session, &body.request_id))
                .expect("string tuple serializes")
        )
    );
    let snapshot = ExecutionSnapshot {
        version: 1,
        assistant_id: "unconfigured".into(),
        assistant_revision: 0,
        model: ModelSnapshot {
            config_id: "unconfigured".into(),
            revision: 0,
            adapter: "unconfigured".into(),
            settings: json!({}),
        },
        capabilities: vec![],
        max_consecutive_identical_calls: 3,
        requires_execution: true,
    };
    let record = storage(state.execution.0.clone(), move |store| {
        store.create_turn(CreateTurn {
            owner: &principal.id,
            session_id: &session,
            expected_session_version: body.expected_session_version,
            turn_id: &format!("turn-{key}"),
            run_id: &format!("run-{key}"),
            input: &body.input,
            snapshot: &snapshot,
        })
    })
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"run":record,"execution_submitted":false,"mode":"state_machine_validation"})),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Start,
    Pause,
    Resume,
    Cancel,
    Fail,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    expected_version: u64,
    action: Action,
}
async fn transition(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path((session, run)): Path<(String, String)>,
    body: Result<Json<Transition>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(body) = body.map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "状态动作不符合接口契约",
        )
    })?;
    let (next, recovery) = match body.action {
        Action::Start | Action::Resume => (RunState::Running, None),
        Action::Pause => (
            RunState::Paused,
            Some(RecoveryDecision {
                action: RecoveryAction::Resume,
                code: "local_state_validation".into(),
                message: "本机状态机验证暂停，未提交外部调用".into(),
            }),
        ),
        Action::Cancel => (RunState::Cancelled, None),
        Action::Fail => (RunState::Failed, None),
    };
    let record = storage(state.execution.0.clone(), move |store| {
        store.transition_run(
            &principal.id,
            &session,
            &run,
            body.expected_version,
            next,
            recovery,
        )
    })
    .await?;
    Ok(Json(json!({"run":record,"execution_submitted":false})))
}
