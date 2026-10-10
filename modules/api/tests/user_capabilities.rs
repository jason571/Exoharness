#[path = "support/identity.rs"]
mod support;
use axum::http::StatusCode;
use exoharness_api::{ApiState, Surface, identity_router_with_registry};
use exoharness_contracts::capability::{CapabilityManifest, ValidatedCapability};
use exoharness_identity::UserCapabilityPolicy;
use exoharness_registry::Registry;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};
use std::sync::Arc;
use support::{fixture, register, request};

#[tokio::test]
async fn personal_catalog_has_real_policy_ownership_and_no_private_configuration() {
    let f = fixture().await;
    let (admin_cookie, _) = support::admin_login(&f).await;
    let (owner, cookie, _) = register(&f).await;
    let database = f.admin_state.execution.0.clone();
    let identity = f.store.clone();
    let expected_owner = owner.clone();
    let database=tokio::task::spawn_blocking(move ||{
        database.migrate_registry().unwrap();
        identity.set_user_capability_policy(&expected_owner,UserCapabilityPolicy{install_mcp:true,..UserCapabilityPolicy::default()}).unwrap();
        for scope in [json!({"kind":"global"}),json!({"kind":"user","user_id":expected_owner}),json!({"kind":"user","user_id":"another-user"})] {
            let manifest:CapabilityManifest=serde_json::from_value(json!({"schema_version":1,"name":"private-test","version":"1.0.0","description":"test","scope":scope,"source":{"kind":"local","path":"/must-not-expose-private-source"},"permissions":[],"secret_refs":[],"spec":{"kind":"skill","config":{"entrypoint":"SKILL.md","content_root":".","executable_scripts":false}}})).unwrap();
            database.register_capability(&RegistryAccess::Administrator,ValidatedCapability::new(manifest,Some("a".repeat(64))).unwrap(),"test",false).unwrap();
        }
        database
    }).await.unwrap();
    let api = identity_router_with_registry(
        ApiState {
            surface: Surface::Portal,
            port: 9798,
            ..f.admin_state.clone()
        },
        f.store,
        f.mailer,
        f.assistants,
        Some(Arc::new(Registry(database))),
    );
    let response = request(
        &api,
        9798,
        "GET",
        "/v1/user-capabilities",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        request(
            &api,
            9798,
            "GET",
            "/v1/user-capabilities",
            Value::Null,
            Some(&admin_cookie),
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(response.body["policy"]["install_mcp"], true);
    assert_eq!(response.body["installation_ready"], false);
    let records = response.body["capabilities"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert!(records[0]["id"].as_str().unwrap().contains(&owner));
    assert_eq!(records[0]["owner"], true);
    assert_eq!(records[0]["status"], "unavailable");
    assert!(!response.body.to_string().contains("must-not-expose"));
    assert_eq!(
        request(
            &api,
            9798,
            "GET",
            "/v1/user-capabilities",
            Value::Null,
            None,
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let jobs = request(
        &api,
        9798,
        "GET",
        "/v1/user-capabilities/install-jobs",
        Value::Null,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(jobs.status, StatusCode::OK);
    assert_eq!(jobs.body, json!({"jobs":[],"installation_ready":false}));
}
