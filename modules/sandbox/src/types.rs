// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Python,
    Shell,
    Javascript,
    Typescript,
}
impl Runtime {
    pub fn valid_entrypoint(self, entry: &str) -> bool {
        crate::filesystem::relative(entry).is_ok() && self.accepts(entry)
    }
    pub(crate) fn command(self, entry: &str) -> Vec<String> {
        match self {
            Self::Python => vec!["/usr/bin/env".into(), "python".into(), entry.into()],
            Self::Shell => vec!["/bin/bash".into(), entry.into()],
            Self::Javascript => vec!["/usr/bin/env".into(), "node".into(), entry.into()],
            Self::Typescript => vec![
                "/usr/bin/env".into(),
                "node".into(),
                "--experimental-transform-types".into(),
                entry.into(),
            ],
        }
    }
    pub(crate) fn accepts(self, entry: &str) -> bool {
        let ext = std::path::Path::new(entry)
            .extension()
            .and_then(|v| v.to_str());
        match self {
            Self::Python => ext == Some("py"),
            Self::Shell => ext == Some("sh"),
            Self::Javascript => matches!(ext, Some("js" | "mjs" | "cjs")),
            Self::Typescript => matches!(ext, Some("ts" | "mts" | "cts")),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Limits {
    pub timeout: Duration,
    pub memory_bytes: u64,
    pub cpu_millis: u32,
    pub pids: u32,
    pub output_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            memory_bytes: 512 * 1024 * 1024,
            cpu_millis: 1000,
            pids: 64,
            output_bytes: 256 * 1024,
        }
    }
}
impl Limits {
    pub(crate) fn validate(&self) -> bool {
        !self.timeout.is_zero()
            && self.timeout <= Duration::from_secs(3600)
            && (32 * 1024 * 1024..=16 * 1024 * 1024 * 1024).contains(&self.memory_bytes)
            && (100..=16000).contains(&self.cpu_millis)
            && (8..=512).contains(&self.pids)
            && (1024..=4 * 1024 * 1024).contains(&self.output_bytes)
    }
}
/// Constructed by the configured workspace store, never deserialized from HTTP.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) store: PathBuf,
    pub(crate) id: String,
}
impl Workspace {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn path(&self) -> &std::path::Path {
        &self.root
    }
}
/// A trusted dispatcher builds this after user/assistant/runtime authorization.
/// The backend does not turn a raw request into an execution grant.
pub struct RunSpec {
    pub invocation_id: String,
    pub workspace: Workspace,
    pub read_only_package: Option<Workspace>,
    pub runtime: Runtime,
    pub entrypoint: String,
    pub arguments: Vec<String>,
    pub limits: Limits,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub invocation_id: String,
    pub image_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub output_truncated: bool,
    pub cleanup_confirmed: bool,
}
#[derive(Debug, Clone, thiserror::Error)]
pub enum SandboxError {
    #[error("{0}")]
    PatchRejected(String),
    #[error("工作区版本已变化；请读取当前版本后局部修改，未执行")]
    StaleRevision,
    #[error("局部修改必须唯一匹配且互不重叠；文件未改变")]
    AmbiguousPatch,
    #[error("沙盒配置或路径不符合执行边界；未执行")]
    Invalid,
    #[error("运行类型和脚本扩展名不匹配；未执行")]
    RuntimeMismatch,
    #[error("当前工作区已有隔离执行，新的调用尚未提交")]
    WorkspaceBusy,
    #[error("沙盒引擎或已准备镜像不可用；不会回退宿主机执行")]
    Unavailable,
    #[error("该调用已有持久执行意图，必须核对原任务而不能重新提交")]
    OutcomeUnknown,
    #[error("沙盒私有工作文件不可用；未确认提交")]
    Storage,
    #[error("沙盒关闭尚未确认，资源保持待清理状态")]
    CleanupPending,
}
