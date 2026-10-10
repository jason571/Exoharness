// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::Connection;
use crate::{McpError, Progress, Result};
use rmcp::{
    model::{CallToolRequest, CallToolRequestParams, ClientRequest},
    service::{PeerRequestOptions, ServiceError},
};
use serde_json::Value;
use std::future::Future;
use tokio::sync::broadcast;
impl Connection {
    pub async fn call<F, C>(
        &self,
        name: &str,
        arguments: Value,
        mut progress: F,
        cancelled: C,
    ) -> Result<Value>
    where
        F: FnMut(Progress),
        C: Future<Output = ()>,
    {
        let Some(arguments) = arguments.as_object().cloned() else {
            return Err(McpError::Arguments);
        };
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(McpError::Arguments);
        }
        tokio::pin!(cancelled);
        let _guard = tokio::select! {biased;() = &mut cancelled=>return Err(McpError::Cancelled),guard=self.calls.lock()=>guard};
        let mut receiver = self.service.service().progress.subscribe();
        let params = CallToolRequestParams::new(name.to_owned()).with_arguments(arguments);
        let options = PeerRequestOptions::with_timeout(self.options.inactivity_timeout)
            .reset_timeout_on_progress();
        let request = ClientRequest::CallToolRequest(CallToolRequest::new(params));
        let handle = self
            .service
            .peer()
            .send_request_with_option(request, options)
            .await
            .map_err(|_| McpError::OutcomeUnknown)?;
        let expected =
            serde_json::to_value(&handle.progress_token).map_err(|_| McpError::Protocol)?;
        let peer = handle.peer.clone();
        let id = handle.id.clone();
        let response = handle.await_response();
        tokio::pin!(response);
        let mut progress_open = true;
        let result=loop {
            tokio::select! {biased;
                result=&mut response=>break result,
                () = &mut cancelled=>{
                    let _=tokio::time::timeout(std::time::Duration::from_secs(3),peer.notify_cancelled(rmcp::model::CancelledNotificationParam::new(Some(id),Some("Run cancelled".into())))).await;
                    return Err(McpError::Cancelled);
                }
                update=receiver.recv(),if progress_open=>match update {
                    Ok(update) if update.token==expected=>progress(update),
                    Ok(_)|Err(broadcast::error::RecvError::Lagged(_))=>{},
                    Err(broadcast::error::RecvError::Closed)=>progress_open=false,
                }
            }
        }.map_err(|error|match error {ServiceError::McpError(error)=>McpError::Rejected(error.code.0),_=>McpError::OutcomeUnknown})?;
        let value = serde_json::to_value(result).map_err(|_| McpError::Protocol)?;
        if serde_json::to_vec(&value)
            .map_err(|_| McpError::Protocol)?
            .len()
            > self.options.max_result_bytes
        {
            return Err(McpError::Protocol);
        }
        Ok(value)
    }
}
