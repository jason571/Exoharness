// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Blocking jobs retain admission until they actually finish, even on cancel.
use crate::{Result, RuntimeError};
use std::sync::Arc;
use tokio::sync::{Semaphore, watch};
pub(crate) struct Workers {
    pub stopped: watch::Sender<bool>,
    io: Arc<Semaphore>,
}
impl Workers {
    pub fn new() -> Self {
        let (stopped, _) = watch::channel(false);
        Self {
            stopped,
            io: Arc::new(Semaphore::new(4)),
        }
    }
    pub async fn run<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        if *self.stopped.borrow() {
            return Err(RuntimeError::Unavailable);
        }
        let permit = self
            .io
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| RuntimeError::Unavailable)?;
        if *self.stopped.borrow() {
            return Err(RuntimeError::Unavailable);
        }
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| RuntimeError::Unavailable)?
    }
    pub async fn drain(&self) {
        let _permit = self.io.acquire_many(4).await;
    }
}
