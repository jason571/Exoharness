// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Result, RunSpec, SandboxError, filesystem::digest};
use std::{path::Path, time::Duration};
use tokio::process::{Child, Command};
pub(crate) struct Engine {
    pub image: String,
    pub owner: String,
    pub user: String,
}
impl Engine {
    pub async fn open(root: &Path, image: &str) -> Result<Self> {
        if image.is_empty()
            || image.len() > 256
            || image.starts_with('-')
            || image.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(SandboxError::Invalid);
        }
        let id = control(&["image", "inspect", "--format", "{{.Id}}", image]).await?;
        let id = id.trim();
        if id.len() != 71
            || !id.starts_with("sha256:")
            || !id[7..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(SandboxError::Unavailable);
        }
        #[cfg(unix)]
        let user = {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::metadata(root).map_err(|_| SandboxError::Storage)?;
            if meta.uid() == 0 {
                return Err(SandboxError::Invalid);
            }
            format!("{}:{}", meta.uid(), meta.gid())
        };
        #[cfg(not(unix))]
        return Err(SandboxError::Unavailable);
        #[cfg(unix)]
        Ok(Self {
            image: id.into(),
            owner: digest(&root.to_string_lossy()),
            user,
        })
    }
    pub fn name(&self, id: &str) -> String {
        format!("exoharness-pro-job-{}-{}", &self.owner[..16], digest(id))
    }
    pub async fn create(&self, spec: &RunSpec) -> Result<String> {
        let name = self.name(&spec.invocation_id);
        let work = spec.workspace.root.to_str().ok_or(SandboxError::Invalid)?;
        let mut args = vec![
            "create".into(),
            "--pull=never".into(),
            "--name".into(),
            name.clone(),
            "--label".into(),
            format!("exoharness_owner={}", self.owner),
            "--read-only".into(),
            "--network".into(),
            "none".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
            "--user".into(),
            self.user.clone(),
            "--ipc".into(),
            "none".into(),
            "--pids-limit".into(),
            spec.limits.pids.to_string(),
            "--memory".into(),
            spec.limits.memory_bytes.to_string(),
            "--memory-swap".into(),
            spec.limits.memory_bytes.to_string(),
            "--cpus".into(),
            format!(
                "{}.{:03}",
                spec.limits.cpu_millis / 1000,
                spec.limits.cpu_millis % 1000
            ),
            "--workdir".into(),
            "/work".into(),
            "--mount".into(),
            format!("type=bind,src={work},dst=/work"),
            "--mount".into(),
            format!("type=bind,src={work},dst=/workspace"),
            "--tmpfs".into(),
            "/tmp:rw,nosuid,nodev,noexec,size=67108864,mode=1777".into(),
            "--init".into(),
            "--entrypoint".into(),
            "/usr/bin/env".into(),
        ];
        if let Some(package) = &spec.read_only_package {
            for destination in ["/package", "/skill", "/work/skill", "/workspace/skill"] {
                args.extend([
                    "--mount".into(),
                    format!(
                        "type=bind,src={},dst={destination},readonly",
                        package.root.to_str().ok_or(SandboxError::Invalid)?
                    ),
                ]);
            }
        }
        args.push(self.image.clone());
        let command = spec.runtime.command(&format!("/work/{}", spec.entrypoint));
        // Overridden entrypoint is always env; even shell source is a FILE
        // argument inside OCI, never passed to a host shell command string.
        args.extend(command);
        args.extend(spec.arguments.clone());
        let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
        control(&borrowed).await?;
        Ok(name)
    }
    pub fn attach(&self, name: &str) -> Result<Child> {
        Command::new("docker")
            .args(["start", "--attach", name])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| SandboxError::Unavailable)
    }
    pub async fn remove(&self, name: &str) -> Result<()> {
        if !self.exists(name).await? {
            return Ok(());
        }
        // Never remove a pre-existing foreign container on a name conflict.
        let owner = control(&[
            "inspect",
            "--format",
            "{{index .Config.Labels \"exoharness_owner\"}}",
            name,
        ])
        .await
        .map_err(|_| SandboxError::CleanupPending)?;
        if owner.trim() != self.owner {
            return Err(SandboxError::CleanupPending);
        }
        control(&["rm", "--force", name])
            .await
            .map(|_| ())
            .map_err(|_| SandboxError::CleanupPending)
    }
    async fn exists(&self, name: &str) -> Result<bool> {
        let filter = format!("name=^/{name}$");
        let names = control(&[
            "container",
            "ls",
            "--all",
            "--filter",
            &filter,
            "--format",
            "{{.Names}}",
        ])
        .await
        .map_err(|_| SandboxError::CleanupPending)?;
        match names.trim() {
            "" => Ok(false),
            found if found == name => Ok(true),
            _ => Err(SandboxError::CleanupPending),
        }
    }
    /// Called only while holding the private provider's exclusive OS lease.
    /// A reservation remains unknown without a receipt; recovery never runs it.
    pub async fn recover(&self, root: &Path) -> Result<()> {
        let label = format!("label=exoharness_owner={}", self.owner);
        let names = control(&[
            "container",
            "ls",
            "--all",
            "--filter",
            &label,
            "--format",
            "{{.Names}}",
        ])
        .await
        .map_err(|_| SandboxError::CleanupPending)?;
        let prefix = format!("exoharness-pro-job-{}-", &self.owner[..16]);
        for name in names.lines() {
            let id = name
                .strip_prefix(&prefix)
                .ok_or(SandboxError::CleanupPending)?;
            crate::filesystem::reserved_digest(root, id)
                .map_err(|_| SandboxError::CleanupPending)?;
            self.remove(name).await?;
        }
        Ok(())
    }
    pub async fn terminate(&self, name: &str) -> Result<()> {
        let owner = control(&[
            "inspect",
            "--format",
            "{{index .Config.Labels \"exoharness_owner\"}}",
            name,
        ])
        .await
        .map_err(|_| SandboxError::CleanupPending)?;
        if owner.trim() != self.owner {
            return Err(SandboxError::CleanupPending);
        }
        control(&["kill", name])
            .await
            .map(|_| ())
            .map_err(|_| SandboxError::CleanupPending)
    }
}
async fn control(args: &[&str]) -> Result<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new("docker")
            .args(args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| SandboxError::Unavailable)?
    .map_err(|_| SandboxError::Unavailable)?;
    if !output.status.success() || output.stdout.len() > 16384 || output.stderr.len() > 16384 {
        return Err(SandboxError::Unavailable);
    }
    String::from_utf8(output.stdout).map_err(|_| SandboxError::Unavailable)
}
