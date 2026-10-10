mod support;
use exoharness_contracts::capability::{
    CapabilitySpec, EnvironmentBinding, McpTransport, PermissionDecision, SecretReference,
};
use exoharness_mcp::{Connection, ConnectionOptions, McpError};
use serde_json::json;
use std::{future::pending, time::Duration};
use support::{StateData, record, secrets, server};
#[tokio::test]
async fn standard_initialize_pagination_and_native_results_have_no_business_rewriting() {
    let server = server(StateData::default()).await;
    let (_root, secrets) = secrets();
    let (record, permissions) = record(&server.url);
    let client = Connection::connect(
        &record,
        &permissions,
        &secrets,
        ConnectionOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(client.server_info()["serverInfo"]["name"], "fixture-server");
    let tools = client.tools().await.unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "echo");
    let result = client
        .call("echo", json!({"text":"中文"}), |_| {}, pending())
        .await
        .unwrap();
    assert_eq!(result["structuredContent"]["echo"]["text"], "中文");
    assert_eq!(result["_meta"]["provider_defined"], "kept");
    let requests = server.state.requests.lock().unwrap().clone();
    assert_eq!(requests[0]["method"], "initialize");
    assert!(
        requests
            .iter()
            .any(|r| r["method"] == "notifications/initialized")
    );
    assert!(
        requests
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .all(|r| r["params"]["_meta"]["progressToken"].is_number())
    );
    client.close().await.unwrap();
}
#[tokio::test]
async fn denied_or_unenabled_records_never_connect_and_stdio_never_spawns_on_host() {
    let server = server(StateData::default()).await;
    let (_root, secrets) = secrets();
    let (mut record, mut permissions) = record(&server.url);
    record.enabled = false;
    assert!(matches!(
        Connection::connect(
            &record,
            &permissions,
            &secrets,
            ConnectionOptions::default()
        )
        .await,
        Err(McpError::Permission)
    ));
    record.enabled = true;
    permissions[0].decision = PermissionDecision::Deny;
    assert!(matches!(
        Connection::connect(
            &record,
            &permissions,
            &secrets,
            ConnectionOptions::default()
        )
        .await,
        Err(McpError::Permission)
    ));
    permissions[0].decision = PermissionDecision::Allow;
    if let CapabilitySpec::McpServer(spec) = &mut record.manifest.spec {
        spec.transport = McpTransport::Stdio {
            command: vec!["must-not-spawn".into()],
            environment: Default::default(),
            working_directory: None,
        };
    }
    assert!(matches!(
        Connection::connect(
            &record,
            &permissions,
            &secrets,
            ConnectionOptions::default()
        )
        .await,
        Err(McpError::SandboxRequired)
    ));
    assert!(server.state.requests.lock().unwrap().is_empty());
}
#[tokio::test]
async fn secret_header_is_exact_and_never_added_to_manifest_or_diagnostics() {
    let server = server(StateData::default()).await;
    let (_root, secrets) = secrets();
    let key = secrets.put("Token fixture-value").unwrap();
    let (mut record, permissions) = record(&server.url);
    record.manifest.secret_refs.push(SecretReference {
        name: "auth".into(),
        provider: "local".into(),
        key,
    });
    if let CapabilitySpec::McpServer(spec) = &mut record.manifest.spec
        && let McpTransport::StreamableHttp { headers, .. } = &mut spec.transport
    {
        headers.insert(
            "Authorization".into(),
            EnvironmentBinding::SecretRef {
                reference: "auth".into(),
            },
        );
    }
    let client = Connection::connect(
        &record,
        &permissions,
        &secrets,
        ConnectionOptions::default(),
    )
    .await
    .unwrap();
    client.tools().await.unwrap();
    assert!(
        server
            .state
            .headers
            .lock()
            .unwrap()
            .iter()
            .all(|headers| headers["authorization"] == "Token fixture-value")
    );
    assert!(
        !serde_json::to_string(&record)
            .unwrap()
            .contains("fixture-value")
    );
    client.close().await.unwrap();
}
#[tokio::test]
async fn real_progress_resets_inactivity_and_wrong_tokens_do_not() {
    for matching in [true, false] {
        let server = server(StateData {
            progress_matches: matching,
            ..Default::default()
        })
        .await;
        let (_root, secrets) = secrets();
        let (record, permissions) = record(&server.url);
        let options = ConnectionOptions {
            inactivity_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let client = Connection::connect(&record, &permissions, &secrets, options)
            .await
            .unwrap();
        let mut updates = Vec::new();
        let result = client
            .call(
                "long",
                json!({}),
                |progress| updates.push(progress),
                pending(),
            )
            .await;
        if matching {
            assert!(result.is_ok(), "{result:?}");
            assert!(!updates.is_empty());
            assert_eq!(updates.last().unwrap().total, Some(6.0));
        } else {
            assert!(matches!(result, Err(McpError::OutcomeUnknown)));
            assert!(updates.is_empty());
        }
        client.close().await.unwrap();
    }
}
#[tokio::test]
async fn cancellation_targets_request_and_never_resubmits_unknown_results() {
    let server = server(StateData {
        progress_matches: true,
        ..Default::default()
    })
    .await;
    let (_root, secrets) = secrets();
    let (record, permissions) = record(&server.url);
    let client = Connection::connect(
        &record,
        &permissions,
        &secrets,
        ConnectionOptions::default(),
    )
    .await
    .unwrap();
    let result = client
        .call(
            "long",
            json!({}),
            |_| {},
            tokio::time::sleep(Duration::from_millis(75)),
        )
        .await;
    assert!(matches!(result, Err(McpError::Cancelled)));
    tokio::time::sleep(Duration::from_millis(30)).await;
    let requests = server.state.requests.lock().unwrap().clone();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        1
    );
    let call = requests
        .iter()
        .find(|r| r["method"] == "tools/call")
        .unwrap();
    let cancel = requests
        .iter()
        .find(|r| r["method"] == "notifications/cancelled")
        .unwrap();
    assert_eq!(cancel["params"]["requestId"], call["id"]);
    client.close().await.unwrap();
}
#[tokio::test]
async fn truncated_stream_and_repeated_pagination_fail_without_unbounded_retry() {
    for config in [
        StateData {
            fail_call: true,
            ..Default::default()
        },
        StateData {
            repeated_cursor: true,
            ..Default::default()
        },
    ] {
        let server = server(config).await;
        let (_root, secrets) = secrets();
        let (record, permissions) = record(&server.url);
        let options = ConnectionOptions {
            inactivity_timeout: Duration::from_millis(120),
            ..Default::default()
        };
        let client = Connection::connect(&record, &permissions, &secrets, options)
            .await
            .unwrap();
        if server.state.fail_call {
            assert!(matches!(
                client.call("echo", json!({}), |_| {}, pending()).await,
                Err(McpError::OutcomeUnknown)
            ));
        } else {
            assert!(matches!(client.tools().await, Err(McpError::Protocol)));
        }
        assert!(server.state.requests.lock().unwrap().len() < 8);
        client.close().await.unwrap();
    }
}
