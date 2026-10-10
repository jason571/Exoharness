// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::Connection;
use crate::{McpError, Result};
use serde_json::{Value, json};
use tokio::time::timeout;
impl Connection {
    pub async fn tools(&self) -> Result<Vec<Value>> {
        timeout(self.options.discovery_timeout, async {
            let mut cursor = None;
            let mut seen = std::collections::HashSet::new();
            let mut names = std::collections::HashSet::new();
            let mut values = Vec::new();
            let mut size = 0usize;
            for _ in 0..64 {
                let params = cursor.take().map(|cursor| {
                    let mut params = rmcp::model::PaginatedRequestParams::default();
                    params.cursor = Some(cursor);
                    params
                });
                let result = self
                    .service
                    .list_tools(params)
                    .await
                    .map_err(|_| McpError::Connection)?;
                for tool in result.tools {
                    if !names.insert(tool.name.to_string()) || values.len() >= 1024 {
                        return Err(McpError::Protocol);
                    }
                    let value = json!(tool);
                    size = size
                        .checked_add(
                            serde_json::to_vec(&value)
                                .map_err(|_| McpError::Protocol)?
                                .len(),
                        )
                        .ok_or(McpError::Protocol)?;
                    if size > self.options.max_result_bytes {
                        return Err(McpError::Protocol);
                    }
                    values.push(value);
                }
                let Some(next) = result.next_cursor else {
                    return Ok(values);
                };
                if !seen.insert(next.clone()) {
                    return Err(McpError::Protocol);
                }
                cursor = Some(next);
            }
            Err(McpError::Protocol)
        })
        .await
        .map_err(|_| McpError::Connection)?
    }
    pub fn server_info(&self) -> Value {
        self.service
            .peer()
            .peer_info()
            .map(|info| json!(info))
            .unwrap_or(Value::Null)
    }
}
