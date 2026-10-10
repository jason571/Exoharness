#[path = "support/catalogs.rs"]
#[allow(dead_code)]
mod support;
use exoharness_configuration::Models;
use serde_json::json;
use support::Fixture;

#[test]
fn omitted_old_api_defaults_are_returned_persisted_and_bound_without_changing_explicit_values() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let provider =
        json!({"name":"api","api":"openai_compatible","base_url":"https://example.test/v1"});
    let entry =
        json!({"name":"minimal","provider":"api","model_id":"fixture","display_name":"Minimal"});
    let saved = models
        .save_configuration(provider, entry, Some("private-fixture-key"), None)
        .unwrap();
    assert_eq!(saved["model"]["context_window"], 128_000);
    assert_eq!(saved["model"]["max_tokens"], 16_384);
    assert_eq!(saved["model"]["thinking"], "none");
    assert_eq!(saved["model"]["enabled"], true);
    assert_eq!(saved["model"]["model_type"], "llm");
    assert_eq!(saved["provider"]["enabled"], true);
    assert_eq!(Models::open(&root.0).unwrap().catalog(), models.catalog());
    let binding = models.bind_enabled("minimal").unwrap();
    assert_eq!(binding.model["context_window"], 128_000);
    let mut entry = saved["model"].clone();
    entry["context_window"] = json!(32000);
    entry["max_tokens"] = json!(100);
    models.save_model(entry.clone(), None).unwrap();
    assert_eq!(models.catalog()["models"][0]["max_tokens"], 100);
    let previous = models.catalog();
    entry["context_window"] = json!(null);
    assert!(models.save_model(entry.clone(), None).is_err());
    entry["context_window"] = json!("32000");
    assert!(models.save_model(entry, None).is_err());
    assert_eq!(models.catalog(), previous);
}
#[test]
fn sparse_import_is_normalized_only_in_projection_and_never_rewrites_the_source() {
    let root = Fixture::new();
    let value = json!({"providers":[{"name":"api","api":"openai_compatible","base_url":"https://example.test/v1"}],"models":[{"name":"minimal","provider":"api","model_id":"fixture","display_name":"Minimal"}],"default_model":"minimal","default_models":{"llm":"minimal"}});
    root.write("models.json", &value);
    let previous = std::fs::read(root.0.join("models.json")).unwrap();
    let models = Models::open(&root.0).unwrap();
    assert_eq!(models.catalog()["models"][0]["context_window"], 128_000);
    assert_eq!(models.catalog()["providers"][0]["enabled"], true);
    assert_eq!(std::fs::read(root.0.join("models.json")).unwrap(), previous);
    models.refresh().unwrap();
    assert_eq!(std::fs::read(root.0.join("models.json")).unwrap(), previous);
    assert!(!root.0.join("models-current.json").exists());
}
