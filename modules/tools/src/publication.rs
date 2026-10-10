// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Result, ToolError, ToolRuntime, sandbox::safe_output};
use serde_json::{Value, json};
use std::path::Path;
impl ToolRuntime {
    pub(crate) async fn publish_outputs(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        root: &Path,
        outputs: Vec<String>,
    ) -> Result<Vec<Value>> {
        // Validate the whole selection before publishing any artifact.
        let sources = outputs
            .into_iter()
            .map(|path| {
                if path.split('/').next() == Some("skill") {
                    return Err(ToolError::Input("技能包文件不能作为任务成品交付".into()));
                }
                let file = safe_output(root, &path)?;
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| ToolError::Input("成品路径无效".into()))?
                    .to_owned();
                Ok((name, file))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut artifacts = Vec::new();
        for (name, source) in sources {
            let service = self.artifacts.clone();
            let (owner, session, run) = (owner.to_owned(), session.to_owned(), run.to_owned());
            let published = tokio::task::spawn_blocking(move || {
                service.publish_file(&owner, &session, &run, &name, source)
            })
            .await
            .map_err(|_| ToolError::Unknown)?
            .map_err(|_| ToolError::Unknown)?;
            let mut card =
                serde_json::to_value(&published.artifact).map_err(|_| ToolError::Storage)?;
            card["url"] = json!(published.url);
            card["download_url"] = json!(published.download_url);
            card["preview_url"] = json!(published.preview_url);
            card["kind"] = json!(match published
                .artifact
                .content_type
                .split('/')
                .next()
                .unwrap_or("")
            {
                "image" => "image",
                "audio" => "audio",
                "video" => "video",
                _ => "file",
            });
            artifacts.push(card);
        }
        Ok(artifacts)
    }
}
