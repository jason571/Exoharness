// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    ApiState,
    auth::Principal,
    errors::{ApiError, storage},
    routes::owner,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Deserialize, Default)]
pub(crate) struct Cursor {
    owner: Option<String>,
    after: Option<u64>,
    limit: Option<usize>,
}

pub(crate) async fn page(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path((session, run)): Path<(String, String)>,
    Query(cursor): Query<Cursor>,
) -> Result<Json<Value>, ApiError> {
    let scope = owner(&state, &principal, cursor.owner)?;
    let events = storage(state.execution.0.clone(), move |store| {
        store.events(
            &scope,
            &session,
            &run,
            cursor.after.unwrap_or(0),
            cursor.limit.unwrap_or(100),
        )
    })
    .await?;
    Ok(Json(json!({"events":events})))
}

pub(crate) async fn stream(
    State(state): State<ApiState>,
    Extension(principal): Extension<Principal>,
    Path((session, run)): Path<(String, String)>,
    Query(cursor): Query<Cursor>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let scope = owner(&state, &principal, cursor.owner)?;
    let first_scope = scope.clone();
    let first_session = session.clone();
    let first_run = run.clone();
    let after = cursor.after.unwrap_or(0);
    // Validate scope/cursor before committing HTTP 200 or spawning a worker.
    let first = storage(state.execution.0.clone(), move |store| {
        store.events(&first_scope, &first_session, &first_run, after, 100)
    })
    .await?;
    let (sender, receiver) = mpsc::channel::<Result<Event, Infallible>>(32);
    let mut stop = state.stop.clone();
    tokio::spawn(async move {
        let mut next = after;
        let mut batch = first;
        loop {
            if *stop.borrow() {
                return;
            }
            for envelope in batch {
                let Ok(event) = Event::default()
                    .id(envelope.sequence.to_string())
                    .event("execution")
                    .json_data(&envelope)
                else {
                    return;
                };
                tokio::select! {result=sender.send(Ok(event))=>if result.is_err(){return;}, _=sender.closed()=>return, _=stop.changed()=>return}
                next = envelope.sequence;
            }
            tokio::select! {_=sender.closed()=>return,_=stop.changed()=>return,_=tokio::time::sleep(Duration::from_millis(300))=>{}}
            let scope = scope.clone();
            let session = session.clone();
            let run = run.clone();
            match storage(state.execution.0.clone(), move |store| {
                store.events(&scope, &session, &run, next, 100)
            })
            .await
            {
                Ok(events) => batch = events,
                Err(error) => {
                    let event = Event::default().event("stream_error").data(
                        json!({"code":error.1,"message":error.2,"resumable":false}).to_string(),
                    );
                    // Do not strand a worker behind a disconnected consumer.
                    tokio::select! {_=sender.send(Ok(event))=>{},_=sender.closed()=>{},_=stop.changed()=>{}}
                    return;
                }
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(receiver)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("alive"),
    ))
}
