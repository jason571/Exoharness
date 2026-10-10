// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::auth::Credentials;
use exoharness_core::services::Service;
use exoharness_storage_postgres::PostgresStore;
use std::sync::Arc;
use tokio::sync::watch;

pub struct ExecutionService(pub PostgresStore);
impl Service for ExecutionService {
    const KEY: &'static str = "execution-store";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Management,
    Portal,
}

#[derive(Clone)]
pub struct ApiState {
    pub execution: Arc<ExecutionService>,
    pub credentials: Arc<Credentials>,
    pub surface: Surface,
    pub port: u16,
    pub stop: watch::Receiver<bool>,
}
