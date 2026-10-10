// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Own every chat worker until completion or awaited shutdown. No detached runs.
use exoharness_core::services::Service;
use exoharness_model_runtime::CancellationToken;
use std::{collections::HashMap, future::Future, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};

struct Job {
    cancel: CancellationToken,
    task: JoinHandle<()>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}
#[derive(Default)]
struct State {
    closed: bool,
    jobs: HashMap<String, Job>,
}
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TaskAdmissionError {
    #[error("chat task service is closed")]
    Closed,
    #[error("run already has an owned worker")]
    Duplicate,
    #[error("owned chat worker capacity reached")]
    Capacity,
}
pub struct ChatTasks {
    state: Mutex<State>,
    shutdown: Mutex<()>,
    capacity: usize,
    drain_timeout: Duration,
}
impl Service for ChatTasks {
    const KEY: &'static str = "chat-tasks";
}
impl Default for ChatTasks {
    fn default() -> Self {
        // Includes queued workers as well as admitted Agent runs.
        Self::new(4096, Duration::from_secs(10)).expect("valid task policy")
    }
}
impl ChatTasks {
    pub fn new(capacity: usize, drain_timeout: Duration) -> Result<Self, &'static str> {
        if capacity == 0 || drain_timeout.is_zero() {
            return Err("task capacity and drain timeout must be positive");
        }
        Ok(Self {
            state: Mutex::new(State::default()),
            shutdown: Mutex::new(()),
            capacity,
            drain_timeout,
        })
    }
    pub async fn is_open(&self) -> bool {
        !self.state.lock().await.closed
    }
    pub async fn spawn<F>(
        &self,
        run: String,
        cancel: CancellationToken,
        worker: F,
    ) -> Result<(), TaskAdmissionError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut state = self.state.lock().await;
        if state.closed {
            return Err(TaskAdmissionError::Closed);
        }
        state.jobs.retain(|_, job| !job.task.is_finished());
        if state.jobs.contains_key(&run) {
            return Err(TaskAdmissionError::Duplicate);
        }
        if state.jobs.len() >= self.capacity {
            return Err(TaskAdmissionError::Capacity);
        }
        state.jobs.insert(
            run,
            Job {
                cancel,
                task: tokio::spawn(worker),
            },
        );
        Ok(())
    }
    pub async fn cancel(&self, run: &str) {
        if let Some(job) = self.state.lock().await.jobs.get(run) {
            job.cancel.cancel();
        }
    }
    /// All concurrent callers wait for the same drain. A deadline aborts and
    /// then joins unfinished tasks; dropping this future still drops Job guards.
    pub async fn close(&self) -> Vec<String> {
        let _shutdown = self.shutdown.lock().await;
        let mut state = self.state.lock().await;
        state.closed = true;
        for job in state.jobs.values() {
            job.cancel.cancel();
        }
        let owned = state.jobs.keys().cloned().collect();
        let deadline = tokio::time::Instant::now() + self.drain_timeout;
        for job in state.jobs.values_mut() {
            if job.task.is_finished() {
                continue;
            }
            if tokio::time::timeout_at(deadline, &mut job.task)
                .await
                .is_err()
            {
                job.task.abort();
                let _ = (&mut job.task).await;
            }
        }
        state.jobs.clear();
        owned
    }
}
