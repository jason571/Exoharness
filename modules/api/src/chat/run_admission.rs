// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Wait before the first model request; no provider traffic while queued.
use super::ChatState;
use exoharness_contracts::execution::RunState;
use exoharness_model_runtime::CancellationToken;
use exoharness_storage_postgres::StoreError;
use serde_json::json;
use std::time::Duration;

pub(super) async fn start(
    state: &ChatState,
    owner: &str,
    session: &str,
    run: &str,
    cancel: &CancellationToken,
) -> Result<(), ()> {
    let mut stop = state.stop.clone();
    let mut waiting = false;
    loop {
        if cancel.is_cancelled() || *stop.borrow() {
            let (store, owner, run) = (state.store.clone(), owner.to_owned(), run.to_owned());
            let _ = tokio::task::spawn_blocking(move || store.chat_cancel(&owner, &run)).await;
            return Err(());
        }
        let result = {
            let (store, assistants, owner, session, run) = (
                state.store.clone(),
                state.assistants.clone(),
                owner.to_owned(),
                session.to_owned(),
                run.to_owned(),
            );
            tokio::task::spawn_blocking(move || {
            let current = store.run(&owner,&session,&run)?;
            if current.state != RunState::Queued { return Err(StoreError::Conflict); }
            assistants.refresh().map_err(|_| StoreError::NotAuthorized)?;
            if !assistants.detail(&current.snapshot.assistant_id).is_some_and(|profile|
                profile["enabled"]==true && profile["active_revision"].as_u64()==Some(current.snapshot.assistant_revision)) {
                return Err(StoreError::NotAuthorized);
            }
            store.transition_run(&owner,&session,&run,current.version,RunState::Running,None)?;
            if waiting { store.chat_event(&owner,&session,&run,json!({"type":"resource_queue","state":"acquired","message":"排队结束，开始执行"}))?; }
            store.chat_event(&owner,&session,&run,json!({"type":"assistant_start"}))
        }).await.map_err(|_| ())?
        };
        match result {
            Ok(()) => return Ok(()),
            Err(StoreError::CapacityWaiting) => {
                if !waiting {
                    let (store, owner, session, run) = (
                        state.store.clone(),
                        owner.to_owned(),
                        session.to_owned(),
                        run.to_owned(),
                    );
                    tokio::task::spawn_blocking(move || store.chat_event(&owner,&session,&run,json!({"type":"resource_queue","state":"waiting","message":"运行并发已满，正在排队等待"})))
                        .await.map_err(|_| ())?.map_err(|_| ())?;
                    waiting = true;
                }
                tokio::select! {biased;
                    ()=cancel.cancelled()=>{},
                    _=stop.changed()=>{cancel.cancel();},
                    ()=tokio::time::sleep(Duration::from_millis(500))=>{},
                }
            }
            Err(error) => {
                let code = if matches!(error, StoreError::NotAuthorized) {
                    "capability_permission_required"
                } else {
                    "service_unavailable"
                };
                let message = if matches!(error, StoreError::NotAuthorized) {
                    "助理配置或授权已改变，请重新发起任务"
                } else {
                    "无法开始运行，输入与执行记录已保留"
                };
                let (store, owner, session, run) = (
                    state.store.clone(),
                    owner.to_owned(),
                    session.to_owned(),
                    run.to_owned(),
                );
                let _ = tokio::task::spawn_blocking(move || {
                    store.chat_event(&owner,&session,&run,json!({"type":"execution_exception","severity":"error","source":"run_admission","diagnostic":format!("{error:?}"),"recoverable":false}))?;
                    store.finish_chat(&owner, &session, &run, json!({"public_notice":crate::public_chat::notice(code)}), Some(message))
                })
                .await;
                return Err(());
            }
        }
    }
}
