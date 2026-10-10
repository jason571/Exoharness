// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Await registered MCP capacity without additional model requests, tool
//! submissions or retry accounting. Queue state is durable frontend evidence.
use super::{TextDriver, event};
use exoharness_agent::StopReason;
use exoharness_storage_postgres::{InvocationAdmission, StoreError};
use exoharness_tools::ToolRuntime;
use serde_json::json;
use std::time::Duration;

pub(super) async fn claim(
    driver: &mut TextDriver,
    tools: &ToolRuntime,
    capability: &str,
    invocation: &str,
) -> Result<(), StopReason> {
    let catalog = driver.catalog.as_ref().ok_or(StopReason::InvalidProposal)?;
    let limit = tools
        .mcp_concurrency(catalog, &driver.owner, capability)
        .await
        .map_err(|_| StopReason::InputRejected)?;
    let mut waiting = false;
    let mut stop = driver.state.stop.clone();
    loop {
        if driver.cancel.is_cancelled() || *stop.borrow() {
            return Err(StopReason::Cancelled);
        }
        // A queued Run cannot retain a revoked grant or an obsolete revision.
        if waiting {
            let (store, owner, session, run, assistants) = (
                driver.state.store.clone(),
                driver.owner.clone(),
                driver.session.clone(),
                driver.run.clone(),
                driver.state.assistants.clone(),
            );
            let allowed = tokio::task::spawn_blocking(move || {
                let frozen = store.run(&owner, &session, &run).ok()?.snapshot;
                assistants.refresh().ok()?;
                assistants.detail(&frozen.assistant_id).filter(|profile| {
                    profile["enabled"] == true
                        && profile["active_revision"].as_u64() == Some(frozen.assistant_revision)
                })
            })
            .await
            .map_err(|_| StopReason::PersistenceFailed)?;
            if allowed.is_none() {
                driver.failure = Some("助理配置或授权已改变；本次未执行，请开启新的运行");
                return Err(StopReason::InputRejected);
            }
            tools
                .mcp_concurrency(
                    driver.catalog.as_ref().ok_or(StopReason::InvalidProposal)?,
                    &driver.owner,
                    capability,
                )
                .await
                .map_err(|_| StopReason::InputRejected)?;
        }
        let store = driver.state.store.clone();
        let (owner, session, run, id) = (
            driver.owner.clone(),
            driver.session.clone(),
            driver.run.clone(),
            invocation.to_owned(),
        );
        let admission = tokio::task::spawn_blocking(move || {
            if let Some(limit) = limit {
                store.claim_mcp_invocation(&owner, &session, &run, &id, limit)
            } else {
                store
                    .claim_invocation(&owner, &session, &run, &id)
                    .map(|claimed| {
                        if claimed {
                            InvocationAdmission::Claimed
                        } else {
                            InvocationAdmission::Unavailable
                        }
                    })
            }
        })
        .await
        .map_err(|_| StopReason::PersistenceFailed)?
        .map_err(|error| match error {
            StoreError::OutcomeUnknown => StopReason::OutcomeUnknown,
            _ => StopReason::PersistenceFailed,
        })?;
        match admission {
            InvocationAdmission::Claimed => {
                if waiting {
                    event(
                        driver,
                        json!({"type":"resource_queue","state":"acquired",
                        "tool_call_id":invocation,"capability_id":capability,
                        "message":"排队结束，正在启动服务调用"}),
                    )
                    .await?;
                }
                return Ok(());
            }
            InvocationAdmission::Unavailable => return Err(StopReason::OutcomeUnknown),
            InvocationAdmission::Waiting => {
                if !waiting {
                    event(
                        driver,
                        json!({"type":"resource_queue","state":"waiting",
                        "tool_call_id":invocation,"capability_id":capability,
                        "message":"服务正在执行其他任务，正在排队等待"}),
                    )
                    .await?;
                    waiting = true;
                }
                tokio::select! {biased;
                    ()=driver.cancel.cancelled()=>return Err(StopReason::Cancelled),
                    _=stop.changed()=>{driver.cancel.cancel();return Err(StopReason::Cancelled);},
                    ()=tokio::time::sleep(Duration::from_millis(500))=>{},
                }
            }
        }
    }
}
