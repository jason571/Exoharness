// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! One negotiated transport generation. Callers own journal and snapshot leases.
mod calls;
mod connect;
mod discovery;
use crate::progress::Handler;
use rmcp::service::{RoleClient, RunningService};
use std::time::Duration;
use tokio::sync::Mutex;
pub struct Connection {
    service: RunningService<RoleClient, Handler>,
    calls: Mutex<()>,
    options: ConnectionOptions,
}
#[derive(Clone, Debug)]
pub struct ConnectionOptions {
    pub startup_timeout: Duration,
    pub discovery_timeout: Duration,
    pub inactivity_timeout: Duration,
    pub max_result_bytes: usize,
}
impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(30),
            discovery_timeout: Duration::from_secs(30),
            inactivity_timeout: Duration::from_secs(600),
            max_result_bytes: 8 * 1024 * 1024,
        }
    }
}
