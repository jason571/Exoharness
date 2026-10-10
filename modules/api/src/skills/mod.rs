// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Skill HTTP boundary. Import, reads and preparation have separate handlers.
mod errors;
mod imports;
mod reads;
mod requests;
use crate::{ApiState, Surface, auth};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use exoharness_skills::Skills;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

#[derive(Clone)]
struct SkillState {
    skills: Arc<Skills>,
}
pub fn skills_router(api: ApiState, skills: Arc<Skills>) -> Router {
    if api.surface != Surface::Management {
        return Router::new();
    }
    Router::new()
        .route("/v1/imports/skill/preview", post(imports::preview))
        .route("/v1/imports/skill", post(imports::import))
        .route("/v1/imports/skill/package", post(imports::import))
        .route(
            "/v1/capabilities/{id}/skill-instructions",
            get(reads::instructions),
        )
        .route("/v1/capabilities/{id}/skill-files", get(reads::files))
        .layer(DefaultBodyLimit::max(96 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .with_state(SkillState { skills })
}
async fn work<T, F>(state: SkillState, job: F) -> Result<T, errors::SkillHttpError>
where
    T: Send + 'static,
    F: FnOnce(SkillState) -> exoharness_skills::Result<T> + Send + 'static,
{
    static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = LIMIT
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| errors::SkillHttpError(exoharness_skills::SkillError::Io))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job(state)
    })
    .await
    .map_err(|_| errors::SkillHttpError(exoharness_skills::SkillError::Io))?
    .map_err(errors::SkillHttpError)
}
