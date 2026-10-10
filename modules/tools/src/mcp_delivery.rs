//! MCP execution and file delivery have separate outcomes. Fetching a result
//! never calls its generating tool again, nor forwards MCP authentication.
use crate::{Catalog, ToolRuntime};
use exoharness_artifacts::RemoteFile;
use exoharness_contracts::capability::{CapabilitySpec, McpTransport};
use exoharness_model_runtime::CancellationToken;
use serde_json::{Value, json};

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
    ) -> Vec<Value> {
        let mut cards = crate::result_artifacts::collect(value);
        if cards.is_empty() {
            return cards;
        }
        let Ok(record) = self.current(catalog, owner, capability).await else {
            return vec![];
        };
        let CapabilitySpec::McpServer(spec) = record.manifest.spec else {
            return cards;
        };
        let McpTransport::StreamableHttp {
            url: service_url, ..
        } = spec.transport
        else {
            return cards;
        };
        for card in &mut cards {
            let Some(url) = card["url"].as_str() else {
                continue;
            };
            if !exoharness_artifacts::same_service_origin(&service_url, url) {
                continue;
            }
            let input = RemoteFile {
                service_url: &service_url,
                url,
                filename: card["filename"].as_str().unwrap_or(""),
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
                    *card =
                        serde_json::to_value(published.artifact).expect("artifact serialization");
                    card["kind"] = kind;
                    card["url"] = json!(published.url);
                    card["download_url"] = json!(published.download_url);
                    card["preview_url"] = json!(published.preview_url);
                }
                Err(_) => {
                    // Retain source evidence without marking generation failed
                    // or inviting automatic regeneration of completed media.
                    card["delivery_status"] = json!("unavailable");
                }
            }
        }
        cards
    }
}
