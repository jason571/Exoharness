// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use super::Dataset;
use crate::{Result, digest, ensure, evidence};
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::AsyncReadExt;
pub fn normalize_score(value: &Value) -> Result<f64> {
    let score = if let Some(n) = value.get("normalized_score").or(value.get("total_score")) {
        n.as_f64()
    } else if value.get("raw_score").is_some() || value["points"].is_number() {
        value["score"].as_f64()
    } else {
        let maximum = value
            .get("max_score")
            .map(Value::as_f64)
            .unwrap_or(Some(1.0));
        ensure(
            maximum.is_some_and(|v| v > 0.0),
            "Invalid official maximum score",
        )?;
        value["score"].as_f64().map(|v| v / maximum.unwrap_or(1.0))
    };
    let score = score.ok_or_else(|| crate::Error::Invalid("Invalid official score".into()))?;
    ensure((0.0..=1.0).contains(&score), "Official score outside [0,1]")?;
    Ok(score)
}
pub struct Grader {
    pub dataset: Dataset,
    pub image: String,
}
impl Grader {
    pub fn new(dataset: Dataset, image: &str) -> Result<Self> {
        ensure(
            regex::Regex::new(r"^[\w./:-]+@sha256:[0-9a-f]{64}$")
                .expect("pattern")
                .is_match(image),
            "Pin prebuilt grading image by digest",
        )?;
        Ok(Self {
            dataset,
            image: image.into(),
        })
    }
    pub async fn grade(
        &self,
        group: &str,
        split: &str,
        task: &str,
        prediction: &Value,
        output: &Path,
    ) -> Result<Value> {
        self.dataset.path(group, split, task)?;
        let temporary = tempfile::Builder::new()
            .prefix("exo-gdpevo-judge-")
            .tempdir()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o755))?;
        }
        let file = temporary.path().join("prediction.json");
        evidence::write(&file, prediction)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444))?;
        }
        let container = temporary
            .path()
            .file_name()
            .expect("temporary directory")
            .to_string_lossy()
            .to_string();
        let mounted = format!("/benchmark/{group}/{split}_tasks/{task}/eval/eval.sh");
        let mut command = tokio::process::Command::new("docker");
        command
            .args([
                "run",
                "--rm",
                "--name",
                &container,
                "--pull=never",
                "--network=none",
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--pids-limit=64",
                "--memory=256m",
                "--cpus=1",
                "--user=65534:65534",
                "--tmpfs=/tmp:rw,noexec,nosuid,size=16m",
                "-e",
                "PYTHONDONTWRITEBYTECODE=1",
                "--mount",
            ])
            .arg(format!(
                "type=bind,src={},dst=/benchmark/{group},readonly",
                self.dataset.groups.join(group).display()
            ))
            .arg("--mount")
            .arg(format!(
                "type=bind,src={},dst=/prediction.json,readonly",
                file.display()
            ))
            .args([
                "--entrypoint=bash",
                &self.image,
                &mounted,
                "/prediction.json",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        async fn read<R: tokio::io::AsyncRead + Unpin>(stream: R) -> Result<Vec<u8>> {
            let mut raw = Vec::new();
            stream.take(1024 * 1024 + 1).read_to_end(&mut raw).await?;
            ensure(
                raw.len() <= 1024 * 1024,
                "Official grader output bound exceeded",
            )?;
            Ok(raw)
        }
        let execution = tokio::time::timeout(Duration::from_secs(60), async {
            let (a, b, status) = tokio::try_join!(read(stdout), read(stderr), async {
                Ok::<_, crate::Error>(child.wait().await?)
            })?;
            Ok::<_, crate::Error>((a, b, status))
        })
        .await;
        let _ = child.kill().await;
        let _ = tokio::time::timeout(
            Duration::from_secs(15),
            tokio::process::Command::new("docker")
                .args(["rm", "-f", &container])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await;
        let recordpath = output.with_extension("execution.json");
        let (raw, stderr, status) = match execution {
            Ok(Ok(value)) => value,
            error => {
                evidence::write(
                    &recordpath,
                    &json!({"grading_image":self.image,"container":container,"completed":false,"diagnostic_code":"GradingFailedOrTimedOut"}),
                )?;
                return Err(match error {
                    Err(_) => crate::Error::Invalid("Official grader timed out".into()),
                    Ok(Err(error)) => error,
                    _ => unreachable!(),
                });
            }
        };
        evidence::write(
            &recordpath,
            &json!({"grading_image":self.image,"container":container,"completed":true,"returncode":status.code(),"stdout_sha256":crate::contracts::sha256(&raw),"stderr_sha256":crate::contracts::sha256(&stderr)}),
        )?;
        if !status.success() {
            evidence::write_bytes(
                &output.with_extension("diagnostic.txt"),
                &stderr[..stderr.len().min(65536)],
            )?;
            return Err(crate::Error::Invalid(
                "Official grader execution failed".into(),
            ));
        }
        let value: Value = serde_json::from_slice(&raw)?;
        let score = normalize_score(&value)?;
        let receipt = json!({"upstream_revision":self.dataset.revision,"group":group,"split":split,"task":task,"prediction_sha256":digest(prediction),"grading_image":self.image,"score":score,"passed":score==1.0,"official_result":value});
        evidence::write(output, &receipt)?;
        Ok(receipt)
    }
}
