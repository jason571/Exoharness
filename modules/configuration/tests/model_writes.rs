#[path = "support/catalogs.rs"]
#[allow(dead_code)]
mod support;
use exoharness_configuration::{ConfigurationError, Models};
use serde_json::{Value, json};
use support::{Fixture, model};
fn entries() -> (Value, Value) {
    let value = model();
    (value["providers"][0].clone(), value["models"][0].clone())
}
#[test]
fn explicit_upstream_output_default_is_persisted_and_protocol_scoped() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let (mut provider, entry) = entries();
    provider["output_token_parameter"] = json!("omit");
    models
        .save_configuration(provider.clone(), entry, Some("fixture-key"), None)
        .unwrap();
    assert_eq!(
        Models::open(&root.0).unwrap().catalog()["providers"][0]["output_token_parameter"],
        "omit"
    );
    let before = models.catalog();
    provider["api"] = json!("anthropic");
    assert!(models.save_provider(provider, None).is_err());
    assert_eq!(models.catalog(), before);
}
#[test]
fn complete_configuration_rotation_and_restart_leave_original_secrets_immutable() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let original = std::fs::read(root.0.join("models.json")).unwrap();
    let (provider, entry) = entries();
    let saved = models
        .save_configuration(
            provider.clone(),
            entry.clone(),
            Some("first-private-key"),
            Some(0),
        )
        .unwrap();
    assert_eq!(saved["provider"]["api_key_configured"], true);
    assert!(!saved.to_string().contains("first-private-key"));
    assert_eq!(models.credential_count().unwrap(), 1);
    let pointer: Value =
        serde_json::from_slice(&std::fs::read(root.0.join("models-current.json")).unwrap())
            .unwrap();
    let previous = root
        .0
        .join("model-versions")
        .join(pointer["generation"].as_str().unwrap());
    let old_files = std::fs::read_dir(previous.join("secrets/model-providers"))
        .unwrap()
        .map(|v| v.unwrap().path())
        .collect::<Vec<_>>();
    models
        .save_configuration(provider, entry, Some("rotated-private-key"), Some(1))
        .unwrap();
    assert_eq!(models.revision(), 2);
    assert_eq!(
        std::fs::read_to_string(&old_files[0]).unwrap(),
        "first-private-key"
    );
    assert_eq!(std::fs::read(root.0.join("models.json")).unwrap(), original);
    let opened = Models::open(&root.0).unwrap();
    assert_eq!(opened.revision(), 2);
    assert_eq!(opened.catalog(), models.catalog());
    assert!(!opened.catalog().to_string().contains("private-key"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&old_files[0])
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
}
#[test]
fn invalid_update_cannot_change_pointer_key_or_visible_catalog() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let (provider, entry) = entries();
    models
        .save_configuration(provider.clone(), entry.clone(), Some("working-key"), None)
        .unwrap();
    let before = models.catalog();
    let pointer = std::fs::read(root.0.join("models-current.json")).unwrap();
    let mut bad = entry;
    bad["max_tokens"] = json!("invalid");
    assert!(
        models
            .save_configuration(provider, bad, Some("must-not-replace"), None)
            .is_err()
    );
    assert_eq!(models.catalog(), before);
    assert_eq!(
        std::fs::read(root.0.join("models-current.json")).unwrap(),
        pointer
    );
    assert_eq!(
        Models::open(&root.0).unwrap().credential_count().unwrap(),
        1
    );
}
#[test]
fn model_switch_only_changes_its_type_and_provider_references_block_deletion() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let (provider, entry) = entries();
    models
        .save_configuration(provider, entry.clone(), Some("key"), None)
        .unwrap();
    let mut second = entry.clone();
    second["name"] = json!("another");
    second["display_name"] = json!("Another");
    models.save_model(second, None).unwrap();
    assert_eq!(models.catalog()["default_model"], "another");
    assert_eq!(models.catalog()["models"][0]["enabled"], false);
    let asr = json!({"name":"speech","model_type":"asr","api":"openai_transcription","base_url":"http://127.0.0.1:8796/v1","enabled":true});
    let asr_model = json!({"name":"asr","provider":"speech","model_id":"sensevoice","display_name":"ASR","model_type":"asr","enabled":true,"context_window":1,"max_tokens":1,"thinking":"none"});
    models
        .save_configuration(asr, asr_model, Some("asr-key"), None)
        .unwrap();
    models
        .set_default(entry["name"].as_str().unwrap(), None)
        .unwrap();
    assert_eq!(models.catalog()["default_models"]["asr"], "asr");
    assert_eq!(models.catalog()["models"][2]["enabled"], true);
    assert!(matches!(
        models.remove_provider("api", None),
        Err(ConfigurationError::Conflict)
    ));
    models.remove_model("another", None).unwrap();
    models
        .remove_model(entry["name"].as_str().unwrap(), None)
        .unwrap();
    models.remove_provider("api", None).unwrap();
    assert!(models.catalog()["default_model"].is_null());
    assert_eq!(models.credential_count().unwrap(), 1);
    assert_eq!(models.catalog()["default_models"]["asr"], "asr");
}
#[test]
fn stale_revision_and_untrusted_key_status_cannot_change_configuration() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let (mut provider, entry) = entries();
    provider["api_key_configured"] = json!(true);
    assert!(
        models
            .save_configuration(provider.clone(), entry.clone(), None, None)
            .is_err()
    );
    models
        .save_configuration(provider, entry, Some("real-key"), Some(0))
        .unwrap();
    let before = models.catalog();
    assert!(matches!(
        models.set_model_enabled("测试模型", false, Some(0)),
        Err(ConfigurationError::Conflict)
    ));
    assert_eq!(models.catalog(), before);
    models.set_provider_enabled("api", false, None).unwrap();
    assert!(models.set_default("测试模型", None).is_err());
}
#[test]
fn two_service_instances_merge_from_persisted_current_revision_without_losing_changes() {
    let root = Fixture::new();
    let first = Models::initialize(&root.0).unwrap();
    let second = Models::open(&root.0).unwrap();
    let (provider, entry) = entries();
    first
        .save_configuration(provider, entry.clone(), Some("key"), None)
        .unwrap();
    let mut next = entry;
    next["name"] = json!("second");
    second.save_model(next, Some(1)).unwrap();
    let reopened = Models::open(&root.0).unwrap();
    assert_eq!(reopened.catalog()["models"].as_array().unwrap().len(), 2);
    assert_eq!(reopened.revision(), 2);
    let before = reopened.revision();
    reopened.set_default("second", None).unwrap();
    assert_eq!(reopened.revision(), before);
}
