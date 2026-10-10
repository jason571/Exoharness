#[path = "support/catalogs.rs"]
mod support;
use exoharness_configuration::{Assistants, ConfigurationError, Models, selected_generation};
use serde_json::{Value, json};
use support::{Fixture, assistants, model, snapshot};

#[test]
fn exact_models_defaults_sampling_and_assistant_revisions_survive_read_without_writes() {
    let root = Fixture::new();
    let catalog = model();
    root.write("models.json", &catalog);
    assert_eq!(Models::open(&root.0).unwrap().catalog(), catalog);
    let original = assistants();
    root.write("agent-profiles.json", &original);
    let profiles = Assistants::open(&root.0).unwrap();
    let listed = profiles.list();
    assert_eq!(listed[0]["active_revision"], 2);
    assert_eq!(listed[0]["access"], snapshot(2)["access"]);
    let public = profiles.default_public();
    assert!(public.get("system_prompt").is_none());
    assert!(public.get("access").is_none());
    assert_eq!(
        serde_json::from_slice::<Value>(
            &std::fs::read(root.0.join("agent-profiles.json")).unwrap()
        )
        .unwrap(),
        original
    );
}
#[test]
fn invalid_native_types_duplicates_missing_providers_defaults_and_plaintext_secrets_are_rejected() {
    let root = Fixture::new();
    let paths = [
        ("/providers/0/api_key", json!("never-display-this")),
        ("/providers/0/model_type", json!(1)),
        ("/models/0/max_tokens", json!("16384")),
        ("/models/0/provider", json!("missing")),
        ("/models/0/top_p", json!(0)),
        ("/default_models/llm", json!("missing")),
    ];
    for (path, value) in paths {
        let mut invalid = model();
        if let Some(slot) = invalid.pointer_mut(path) {
            *slot = value
        } else {
            invalid["providers"][0]["api_key"] = value;
        }
        root.write("models.json", &invalid);
        assert!(Models::open(&root.0).is_err(), "{path}");
    }
    root.bytes(
        "models.json",
        br#"{"providers":[],"providers":[],"models":[]}"#,
    );
    assert!(matches!(
        Models::open(&root.0),
        Err(ConfigurationError::Json)
    ));
}
#[test]
fn missing_credential_invalid_assistant_selections_and_missing_revision_fail_closed() {
    let root = Fixture::new();
    let mut catalog = model();
    catalog["providers"][0]["api_key_configured"] = json!(true);
    root.write("models.json", &catalog);
    assert!(Models::open(&root.0).is_err());
    let mut invalid = assistants();
    invalid["profiles"][0]["active_revision"] = json!(9);
    root.write("agent-profiles.json", &invalid);
    assert!(Assistants::open(&root.0).is_err());
    invalid = assistants();
    invalid["profiles"][0]["revisions"][0]["access"]["mcp"] = json!({"mode":"all","ids":["x"]});
    root.write("agent-profiles.json", &invalid);
    assert!(Assistants::open(&root.0).is_err());
    invalid = assistants();
    invalid["profiles"][0]["revisions"][0]["avatar"] =
        json!({"asset_id":"../not-an-asset.png","content_type":"image/png"});
    root.write("agent-profiles.json", &invalid);
    assert!(Assistants::open(&root.0).is_err());
}
#[test]
fn pointer_cannot_escape_and_never_sources_server_environment() {
    let root = Fixture::new();
    assert!(selected_generation(&root.0).unwrap().is_none());
    root.write("current.json", &json!({"generation":"../production"}));
    assert!(selected_generation(&root.0).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/tmp", root.0.join("server-good")).unwrap();
        root.write("current.json", &json!({"generation":"server-good"}));
        assert!(selected_generation(&root.0).is_err());
        root.bytes("models.json", model().to_string().as_bytes());
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.0.join("models.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(Models::open(&root.0).is_err());
    }
}
