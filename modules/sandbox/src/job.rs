// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Receipt, Result, RunSpec, SandboxError, output::capture, service::Inner};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{oneshot, watch},
    task::JoinHandle,
};
pub(crate) struct OwnedJob {
    pub _task: JoinHandle<()>,
    pub cancel: watch::Sender<bool>,
    pub finished: watch::Receiver<bool>,
}
pub struct JobHandle {
    done: Option<oneshot::Receiver<Result<Receipt>>>,
    cancel: Option<watch::Sender<bool>>,
}
impl JobHandle {
    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            let _ = cancel.send(true);
        }
    }
    pub async fn finish(mut self) -> Result<Receipt> {
        let result = self
            .done
            .take()
            .ok_or(SandboxError::Unavailable)?
            .await
            .map_err(|_| SandboxError::CleanupPending)?;
        self.cancel = None;
        result
    }
}
impl Drop for JobHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}
pub(crate) fn spawn(inner: Arc<Inner>, spec: RunSpec) -> Result<JobHandle> {
    let (cancel, rx) = watch::channel(false);
    let (tx, done) = oneshot::channel();
    let (finished, completion) = watch::channel(false);
    let id = spec.invocation_id.clone();
    let mut tasks = inner.tasks.lock().map_err(|_| SandboxError::Unavailable)?;
    let worker = inner.clone();
    let owned_id = id.clone();
    let workspace_id = spec.workspace.id.clone();
    let task = tokio::spawn(async move {
        let result = run(&worker, spec, rx).await;
        if let Ok(mut active) = worker.workspaces.lock() {
            active.remove(&workspace_id);
        }
        let _ = tx.send(result);
        let _ = finished.send(true);
        if let Ok(mut tasks) = worker.tasks.lock() {
            tasks.remove(&owned_id);
        }
    });
    tasks.insert(
        id,
        OwnedJob {
            _task: task,
            cancel: cancel.clone(),
            finished: completion,
        },
    );
    Ok(JobHandle {
        done: Some(done),
        cancel: Some(cancel),
    })
}
async fn run(inner: &Inner, spec: RunSpec, mut cancel: watch::Receiver<bool>) -> Result<Receipt> {
    let name = inner.engine.name(&spec.invocation_id);
    let created = inner.engine.create(&spec).await;
    if created.is_err() {
        // Even failed create may have committed with a lost Docker receipt.
        if inner.engine.remove(&name).await.is_err() {
            inner
                .pending
                .lock()
                .map_err(|_| SandboxError::CleanupPending)?
                .insert(name);
        }
        return Err(SandboxError::OutcomeUnknown);
    }
    let result = execute(inner, &name, &spec, &mut cancel).await;
    let closed = inner.engine.remove(&name).await.is_ok();
    if !closed {
        inner
            .pending
            .lock()
            .map_err(|_| SandboxError::CleanupPending)?
            .insert(name);
    }
    if let Ok(mut receipt) = result {
        receipt.cleanup_confirmed = closed;
        crate::filesystem::save_receipt(&inner.root, &receipt)?;
        if !closed {
            return Err(SandboxError::CleanupPending);
        }
        return Ok(receipt);
    }
    if !closed {
        return Err(SandboxError::CleanupPending);
    }
    result
}
async fn execute(
    inner: &Inner,
    name: &str,
    spec: &RunSpec,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Receipt> {
    if *cancel.borrow() {
        return Ok(receipt(
            inner,
            spec,
            "cancelled",
            None,
            Vec::new(),
            Vec::new(),
            false,
        ));
    }
    let mut child = inner.engine.attach(name)?;
    let stdout = child.stdout.take().ok_or(SandboxError::Unavailable)?;
    let stderr = child.stderr.take().ok_or(SandboxError::Unavailable)?;
    let mut out = tokio::spawn(capture(stdout, spec.limits.output_bytes / 2));
    let mut err = tokio::spawn(capture(stderr, spec.limits.output_bytes / 2));
    let (status, exit) = tokio::select! {
        biased;
        _=cancel.changed()=>{let _=inner.engine.terminate(name).await;let _=child.kill().await;("cancelled",None)},
        _=tokio::time::sleep(spec.limits.timeout)=>{let _=inner.engine.terminate(name).await;let _=child.kill().await;("timed_out",None)},
        result=child.wait()=>{let result=result.map_err(|_|SandboxError::OutcomeUnknown)?;(if result.success(){"completed"}else{"failed"},result.code())},
    };
    let captured = tokio::time::timeout(Duration::from_secs(2), async {
        Ok::<_, SandboxError>((
            (&mut out)
                .await
                .map_err(|_| SandboxError::CleanupPending)?
                .map_err(|_| SandboxError::Storage)?,
            (&mut err)
                .await
                .map_err(|_| SandboxError::CleanupPending)?
                .map_err(|_| SandboxError::Storage)?,
        ))
    })
    .await;
    let (stdout, stderr, truncated) = match captured {
        Ok(Ok(((out, a), (err, b)))) => (out, err, a || b),
        _ => {
            out.abort();
            err.abort();
            (Vec::new(), Vec::new(), true)
        }
    };
    Ok(receipt(
        inner, spec, status, exit, stdout, stderr, truncated,
    ))
}
fn receipt(
    inner: &Inner,
    spec: &RunSpec,
    status: &str,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    output_truncated: bool,
) -> Receipt {
    let (stdout, out_truncated) = decode(stdout, spec.limits.output_bytes / 2);
    let (stderr, err_truncated) = decode(stderr, spec.limits.output_bytes / 2);
    Receipt {
        invocation_id: spec.invocation_id.clone(),
        image_id: inner.engine.image.clone(),
        status: status.into(),
        exit_code,
        stdout,
        stderr,
        output_truncated: output_truncated || out_truncated || err_truncated,
        cleanup_confirmed: false,
    }
}
fn decode(bytes: Vec<u8>, limit: usize) -> (String, bool) {
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    let truncated = text.len() > limit;
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, truncated)
}
