use super::{Connection, ConnectionOptions};
use crate::{McpError, Result, admission, headers, progress::Handler};
use exoharness_contracts::capability::{McpTransport, PermissionRecord, StoredCapability};
use exoharness_secrets::SecretStore;
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use tokio::{
    sync::{Mutex, broadcast},
    time::timeout,
};
impl Connection {
    pub async fn connect(
        record: &StoredCapability,
        permissions: &[PermissionRecord],
        secrets: &SecretStore,
        options: ConnectionOptions,
    ) -> Result<Self> {
        let spec = admission::admit(record, permissions)?;
        if options.startup_timeout.is_zero()
            || options.discovery_timeout.is_zero()
            || options.inactivity_timeout.is_zero()
            || !(1024..=32 * 1024 * 1024).contains(&options.max_result_bytes)
        {
            return Err(McpError::Configuration);
        }
        let McpTransport::StreamableHttp {
            url,
            headers,
            auth_secret,
        } = &spec.transport
        else {
            return Err(McpError::SandboxRequired);
        };
        let headers = headers::resolve(record, headers, auth_secret.as_deref(), secrets)?;
        let mut config =
            StreamableHttpClientTransportConfig::with_uri(url.clone()).custom_headers(headers);
        // An expired session may not replay an unknown tools/call side effect.
        config.reinit_on_expired_session = false;
        config.max_sse_event_size = options.max_result_bytes;
        // No default read timeout: only request inactivity, reset by MCP progress.
        // Redirects cannot leak custom Authorization to another endpoint.
        let http = reqwest::Client::builder()
            .connect_timeout(options.startup_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| McpError::Connection)?;
        let transport = StreamableHttpClientTransport::with_client(http, config);
        let (progress, _) = broadcast::channel(256);
        let service = timeout(
            options.startup_timeout,
            Handler { progress }.serve_with_lifecycle(transport, ClientLifecycleMode::Initialize),
        )
        .await
        .map_err(|_| McpError::Connection)?
        .map_err(|_| McpError::Connection)?;
        if spec.protocol_version.as_ref().is_some_and(|version| {
            service
                .peer()
                .peer_info()
                .is_none_or(|info| info.protocol_version.to_string() != *version)
        }) {
            let _ = service.cancel().await;
            return Err(McpError::Protocol);
        }
        Ok(Self {
            service,
            calls: Mutex::new(()),
            options,
        })
    }
    pub fn is_closed(&self) -> bool {
        self.service.is_closed()
    }
    pub async fn close(mut self) -> Result<()> {
        self.service
            .close()
            .await
            .map(|_| ())
            .map_err(|_| McpError::Connection)
    }
}
