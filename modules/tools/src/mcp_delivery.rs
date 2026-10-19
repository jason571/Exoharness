// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! A file's visibility is independent of durable ownership. Downloading an
//! explicitly declared output never resubmits its generator or forwards auth.
use crate::{Catalog, ToolRuntime};
use exoharness_artifacts::RemoteFile;
use exoharness_contracts::capability::{CapabilitySpec, McpTransport};
use exoharness_model_runtime::CancellationToken;
use serde_json::{Value, json};

#[derive(Default)]
pub(crate) struct Outputs {
    pub cards: Vec<Value>,
    pub saved: Vec<Value>,
}

impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn deliver_mcp_outputs(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        capability: &str,
        cancel: &CancellationToken,
        value: &Value,
    ) -> Outputs {
        let mut outputs = Outputs::default();
        let files = crate::result_artifacts::descriptors(value);
        if files.is_empty() {
            return outputs;
        }
        let Ok(record) = self.current(catalog, owner, capability).await else {
            return outputs;
        };
        let CapabilitySpec::McpServer(spec) = record.manifest.spec else {
            return outputs;
        };
        let endpoint = match &spec.transport {
            McpTransport::StreamableHttp { url, .. } => Some(url),
            _ => None,
        };
        for mut card in files {
            let hidden = crate::result_artifacts::internal(&card);
            let source = card["url"].as_str().unwrap().to_owned();
            // Only the registered endpoint and administrator-declared output
            // origins qualify. A tool cannot expand this list in its result.
            let approved = endpoint
                .into_iter()
                .chain(spec.artifact_origins.iter())
                .find(|origin| exoharness_artifacts::same_service_origin(origin, &source));
            if let Some(origin) = approved {
                let input = RemoteFile {
                    service_url: origin,
                    url: &source,
                    filename: card["filename"].as_str().unwrap(),
                    size: card["size_bytes"].as_u64(),
                    sha256: card["sha256"].as_str(),
                };
                let result = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => break,
                    result = self.artifacts.import_remote(owner, session, run, input) => result,
                };
                match result {
                    Ok(published) => {
                        let kind = card["kind"].clone();
                        card = serde_json::to_value(published.artifact)
                            .expect("artifact serialization");
                        card["kind"] = kind;
                        card["url"] = json!(published.url);
                        card["download_url"] = json!(published.download_url);
                        card["preview_url"] = json!(published.preview_url);
                        card["artifact_id"] = card["id"].clone();
                        if hidden {
                            // Native handles are separate host evidence. Raw
                            // MCP job IDs and signed source links stay intact.
                            card["artifact_visibility"] = json!("internal");
                            card["source_url"] = json!(source);
                            outputs.saved.push(card.clone());
                        }
                    }
                    Err(_) => {
                        card["delivery_status"] = json!("unavailable");
                    }
                }
            }
            if !hidden {
                outputs.cards.push(card);
            }
        }
        outputs
    }
}
