// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::ChatState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use exoharness_contracts::execution::ExecutionEvent;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
pub(super) async fn events(
    State(state): State<ChatState>,
    Extension(principal): Extension<Principal>,
    Path(run): Path<String>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let after = headers
        .get("last-event-id")
        .map(|v| v.to_str().ok().and_then(|v| v.parse::<u64>().ok()))
        .unwrap_or(Some(0))
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_cursor",
            "事件游标不正确",
        ))?;
    let own = principal.id.clone();
    let rid = run.clone();
    let record = storage(state.store.clone(), move |s| s.owned_run(&own, &rid)).await?;
    if after > record.version {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_cursor",
            "事件游标超出当前记录范围",
        ));
    }
    let owner = principal.id;
    let session = record.session_id;
    let (sender, receiver) = mpsc::channel::<Result<Event, Infallible>>(32);
    let mut stop = state.stop.clone();
    tokio::spawn(async move {
        let mut cursor = after;
        loop {
            let own = owner.clone();
            let sid = session.clone();
            let rid = run.clone();
            let batch = storage(state.store.clone(), move |s| {
                s.events(&own, &sid, &rid, cursor, 100)
            })
            .await;
            let Ok(batch) = batch else { return };
            for envelope in batch {
                cursor = envelope.sequence;
                if let ExecutionEvent::Chat { payload } = envelope.event {
                    let terminal = matches!(
                        payload["type"].as_str(),
                        Some("complete" | "failed" | "cancelled" | "recoverable")
                    );
                    let Ok(event) = Event::default()
                        .event("chat")
                        .id(cursor.to_string())
                        .json_data(payload)
                    else {
                        return;
                    };
                    tokio::select! {_=sender.closed()=>return,_=stop.changed()=>return,result=sender.send(Ok(event))=>if result.is_err(){return}}
                    if terminal {
                        return;
                    }
                }
            }
            // A reconnect may already acknowledge the terminal event. Do not
            // leave such clients waiting forever for an event that cannot exist.
            let own = owner.clone();
            let rid = run.clone();
            let Ok(current) = storage(state.store.clone(), move |s| s.owned_run(&own, &rid)).await
            else {
                return;
            };
            if !current.state.active() && cursor >= current.version {
                return;
            }
            if *stop.borrow() {
                return;
            }
            tokio::select! {_=sender.closed()=>return,_=stop.changed()=>return,_=tokio::time::sleep(Duration::from_millis(200))=>{}}
        }
    });
    Ok(Sse::new(ReceiverStream::new(receiver))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10))))
}
