mod support;
use exoharness_mcp::{Connection, ConnectionOptions, McpError};
use serde_json::json;
use std::{
    collections::{BTreeMap, VecDeque},
    future::pending,
    sync::Mutex,
    time::Duration,
};
use support::{StateData, record, secrets, server};

fn fault(method: &str, status: u16) -> StateData {
    StateData {
        http_faults: Mutex::new(BTreeMap::from([(method.into(), VecDeque::from([status]))])),
        ..Default::default()
    }
}
fn options() -> ConnectionOptions {
    ConnectionOptions {
        startup_timeout: Duration::from_millis(300),
        discovery_timeout: Duration::from_millis(300),
        inactivity_timeout: Duration::from_millis(150),
        max_result_bytes: 1024,
    }
}
#[tokio::test]
async fn initialization_faults_submit_no_business_work_or_credentials_in_errors() {
    for status in [401, 403, 404, 429, 500, 502, 503, 504] {
        let remote = server(fault("initialize", status)).await;
        let (_root, secret) = secrets();
        let (record, permissions) = record(&remote.url);
        let result = Connection::connect(&record, &permissions, &secret, options()).await;
        assert!(
            matches!(result, Err(McpError::Connection)),
            "status={status}"
        );
        assert!(
            !remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "tools/call")
        );
    }
}
#[tokio::test]
async fn discovery_faults_are_bounded_and_never_submit_a_tool() {
    for status in [401, 403, 429, 500, 502, 503, 504] {
        let remote = server(fault("tools/list", status)).await;
        let (_root, secret) = secrets();
        let (record, permissions) = record(&remote.url);
        let client = Connection::connect(&record, &permissions, &secret, options())
            .await
            .unwrap();
        assert!(matches!(client.tools().await, Err(McpError::Connection)));
        assert_eq!(
            remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r["method"] == "tools/list")
                .count(),
            1
        );
        assert!(
            !remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "tools/call")
        );
        let _ = client.close().await;
    }
}
#[tokio::test]
async fn post_submission_http_faults_never_replay_unknown_side_effects() {
    for status in [401, 403, 404, 409, 413, 429, 500, 502, 503, 504] {
        let remote = server(fault("tools/call:echo", status)).await;
        let (_root, secret) = secrets();
        let (record, permissions) = record(&remote.url);
        let client = Connection::connect(&record, &permissions, &secret, options())
            .await
            .unwrap();
        let result = client
            .call("echo", json!({"text":"submit once"}), |_| {}, pending())
            .await;
        assert!(
            matches!(result, Err(McpError::OutcomeUnknown)),
            "status={status}"
        );
        assert_eq!(
            remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r["method"] == "tools/call")
                .count(),
            1
        );
        let _ = client.close().await;
    }
}
#[tokio::test]
async fn malformed_mismatched_and_oversized_receipts_cannot_be_claimed_successful() {
    for config in [
        StateData {
            malformed_call: true,
            ..Default::default()
        },
        StateData {
            wrong_call_id: true,
            ..Default::default()
        },
        StateData {
            large_call_bytes: 2048,
            ..Default::default()
        },
    ] {
        let remote = server(config).await;
        let (_root, secret) = secrets();
        let (record, permissions) = record(&remote.url);
        let client = Connection::connect(&record, &permissions, &secret, options())
            .await
            .unwrap();
        let result = client.call("echo", json!({}), |_| {}, pending()).await;
        assert!(matches!(
            result,
            Err(McpError::OutcomeUnknown | McpError::Protocol)
        ));
        assert_eq!(
            remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r["method"] == "tools/call")
                .count(),
            1
        );
        let _ = client.close().await;
    }
}
