#[path = "support/catalogs.rs"]
#[allow(dead_code)]
mod support;
use exoharness_configuration::{Assistants, ConfigurationError};
use serde_json::{Value, json};
use support::Fixture;
fn input() -> Value {
    json!({"display_name":"助理","avatar_text":"助","avatar":null,"description":"Test assistant","system_prompt":"Instructions.\nSecond line.","skill_ids":[],"access":{"skills":{"mode":"none"},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"none"},"published":true}})
}
#[test]
fn conversation_model_is_separate_from_execution_grants_and_missing_field_preserves_it() {
    let root = Fixture::new();
    let service = Assistants::initialize(&root.0).unwrap();
    let mut initial = input();
    initial["default_llm"] = json!("configured-llm");
    service.create("one", initial, true, false).unwrap();
    let original = service.detail("one").unwrap();
    service.update("one", 1, input()).unwrap();
    let next = service.detail("one").unwrap();
    assert_eq!(next["default_llm"], "configured-llm");
    assert_eq!(next["access"], original["access"]);
    let mut clear = input();
    clear["default_llm"] = Value::Null;
    service.update("one", 2, clear).unwrap();
    assert!(service.detail("one").unwrap()["default_llm"].is_null());
    assert!(service.public_profile("one").get("default_llm").is_none());
    let mut invalid = input();
    invalid["default_llm"] = json!({"model":"wrong-shape"});
    assert!(service.update("one", 3, invalid).is_err());
}
#[test]
fn updates_keep_one_identity_and_exact_previous_snapshots_across_restart() {
    let root = Fixture::new();
    let service = Assistants::initialize(&root.0).unwrap();
    let baseline = std::fs::read(root.0.join("agent-profiles.json")).unwrap();
    service.create("one", input(), true, false).unwrap();
    let first = service.detail("one").unwrap();
    let mut next = input();
    next["system_prompt"] = json!("New instructions");
    service.update("one", 1, next).unwrap();
    assert_eq!(service.list().len(), 1);
    assert_eq!(service.detail("one").unwrap()["active_revision"], 2);
    let current: Value =
        serde_json::from_slice(&std::fs::read(root.0.join("agent-profiles-current.json")).unwrap())
            .unwrap();
    assert_eq!(
        current["profiles"][0]["revisions"][0]["system_prompt"],
        first["system_prompt"]
    );
    assert_eq!(
        std::fs::read(root.0.join("agent-profiles.json")).unwrap(),
        baseline
    );
    assert_eq!(Assistants::open(&root.0).unwrap().list(), service.list());
    assert!(matches!(
        service.update("one", 1, input()),
        Err(ConfigurationError::Conflict)
    ));
    assert_eq!(service.detail("one").unwrap()["active_revision"], 2);
}
#[test]
fn avatar_upload_binds_to_current_assistant_and_survives_reload_without_base64() {
    let root = Fixture::new();
    let service = Assistants::initialize(&root.0).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\nopaque-fixture";
    let asset = service.upload_avatar("image/png", bytes).unwrap();
    assert_eq!(service.upload_avatar("image/png", bytes).unwrap(), asset);
    assert!(service.upload_avatar("image/jpeg", bytes).is_err());
    assert!(service.upload_avatar("image/svg+xml", b"<svg/>").is_err());
    assert!(service.avatar("../escape.png").is_none());
    let mut spec = input();
    spec["avatar"] = asset.clone();
    service.create("one", input(), true, false).unwrap();
    service.update("one", 1, spec).unwrap();
    let reopened = Assistants::open(&root.0).unwrap();
    assert_eq!(reopened.list().len(), 1);
    assert_eq!(reopened.detail("one").unwrap()["avatar"], asset);
    assert_eq!(
        reopened
            .avatar(asset["asset_id"].as_str().unwrap())
            .unwrap()
            .1,
        bytes
    );
    assert_eq!(reopened.avatar_count(), 1);
    assert!(
        !std::fs::read_to_string(root.0.join("agent-profiles-current.json"))
            .unwrap()
            .contains("base64")
    );
    service.remove("one", || Ok(false)).unwrap();
    assert_eq!(service.list().len(), 0);
    // Old Run snapshots may still reference the content-addressed avatar.
    assert!(
        reopened
            .avatar(asset["asset_id"].as_str().unwrap())
            .is_some()
    );
}
#[test]
fn malformed_revisions_defaults_missing_assets_and_assignments_do_not_commit() {
    let root = Fixture::new();
    let service = Assistants::initialize(&root.0).unwrap();
    service.create("one", input(), true, true).unwrap();
    let before = std::fs::read(root.0.join("agent-profiles-current.json")).unwrap();
    assert!(service.set_enabled("one", false).is_err());
    assert!(service.remove("one", || Ok(false)).is_err());
    let mut bad = input();
    bad["avatar"] = json!({"asset_id":format!("{}.png","a".repeat(64)),"content_type":"image/png"});
    assert!(service.update("one", 1, bad).is_err());
    assert_eq!(
        before,
        std::fs::read(root.0.join("agent-profiles-current.json")).unwrap()
    );
    service.create("two", input(), false, false).unwrap();
    assert!(service.assignment("two").is_err());
    assert!(service.set_default("two").is_err());
    service.set_enabled("two", true).unwrap();
    assert!(service.remove("two", || Ok(true)).is_err());
    let mut private = input();
    private["access"]["published"] = json!(false);
    service.update("two", 1, private).unwrap();
    assert!(service.assignment("two").is_err());
    assert!(service.public_profile("two").is_null());
}
#[test]
fn cross_instance_assignment_lease_prevents_delete_until_release() {
    let root = Fixture::new();
    let a = Assistants::initialize(&root.0).unwrap();
    a.create("one", input(), true, false).unwrap();
    let b = Assistants::open(&root.0).unwrap();
    let lease = a.assignment("one").unwrap();
    assert_eq!(lease.profile()["id"], "one");
    assert!(matches!(
        b.remove("one", || Ok(false)),
        Err(ConfigurationError::Conflict)
    ));
    drop(lease);
    b.remove("one", || Ok(false)).unwrap();
    a.refresh().unwrap();
    assert!(a.detail("one").is_none());
    assert!(a.assignment("one").is_err());
}
#[test]
fn omitted_access_never_implicitly_grants_mcp_and_updates_preserve_explicit_access() {
    let root = Fixture::new();
    let a = Assistants::initialize(&root.0).unwrap();
    let mut spec = input();
    spec.as_object_mut().unwrap().remove("access");
    a.create("one", spec.clone(), true, false).unwrap();
    assert_eq!(a.detail("one").unwrap()["access"]["mcp"]["mode"], "none");
    let mut explicit = input();
    explicit["access"]["mcp"] = json!({"mode":"selected","ids":["mcp_server:global:_:test"]});
    a.update("one", 1, explicit).unwrap();
    a.update("one", 2, spec).unwrap();
    assert_eq!(
        a.detail("one").unwrap()["access"]["mcp"]["ids"][0],
        "mcp_server:global:_:test"
    );
}
