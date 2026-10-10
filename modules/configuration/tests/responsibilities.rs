#[path = "support/catalogs.rs"]
mod support;
use exoharness_configuration::{Assistants, Models};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{Fixture, assistants, model};

fn audio_catalog() -> Value {
    let mut value = model();
    value["providers"].as_array_mut().unwrap().extend([
        json!({"name":"speech", "api":"volcengine_tts_sse", "model_type":"tts",
            "base_url":"https://speech.example.test/sse", "enabled":true,
            "api_key_configured":false}),
        json!({"name":"transcription", "api":"openai_transcription", "model_type":"asr",
            "base_url":"http://127.0.0.1:8796/v1", "enabled":true,
            "api_key_configured":false}),
    ]);
    value["models"].as_array_mut().unwrap().extend([
        json!({"name":"voice", "model_id":"voice-resource", "display_name":"Voice",
            "provider":"speech", "model_type":"tts", "enabled":true,
            "tts":{"profile_id":"persona", "speaker":"test-voice", "sample_rate":24000,
                "audio_format":"mp3"}}),
        json!({"name":"asr", "model_id":"test-asr", "display_name":"ASR",
            "provider":"transcription", "model_type":"asr", "enabled":true}),
    ]);
    value["default_models"]["tts"] = json!("voice");
    value["default_models"]["asr"] = json!("asr");
    value
}

#[test]
fn independent_model_types_and_defaults_survive_module_separation() {
    let root = Fixture::new();
    let catalog = audio_catalog();
    root.write("models.json", &catalog);
    let original = std::fs::read(root.0.join("models.json")).unwrap();
    let loaded = Models::open(&root.0).unwrap();
    let mut expected = catalog.clone();
    for entry in expected["models"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .skip(1)
    {
        entry["context_window"] = json!(128000);
        entry["max_tokens"] = json!(16384);
        entry["thinking"] = json!("none");
    }
    assert_eq!(loaded.catalog(), expected);
    assert_eq!(std::fs::read(root.0.join("models.json")).unwrap(), original);
    assert_eq!(loaded.credential_count().unwrap(), 0);
    let invalid_values = [
        ("/providers/1/api", json!("openai_transcription")),
        (
            "/providers/2/base_url",
            json!("http://remote.example.test/v1"),
        ),
        ("/models/1/tts/sample_rate", json!("24000")),
        ("/models/1/tts/audio_format", json!("exe")),
        ("/models/1/tts/speaker", json!("")),
        ("/default_models/asr", json!("voice")),
    ];
    for (pointer, replacement) in invalid_values {
        let mut invalid = catalog.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        root.write("models.json", &invalid);
        assert!(Models::open(&root.0).is_err(), "{pointer}");
    }
}

#[test]
fn private_credential_checks_never_modify_or_expose_the_public_catalog() {
    let root = Fixture::new();
    for name in ["secrets", "secrets/model-providers"] {
        let path = root.0.join(name);
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let key_path = format!(
        "secrets/model-providers/model-provider-{:x}.secret",
        Sha256::digest(b"api")
    );
    let key = "fixture-private-key-never-a-response";
    root.bytes(&key_path, key.as_bytes());
    let mut catalog = model();
    catalog["providers"][0]["api_key_configured"] = json!(true);
    root.write("models.json", &catalog);
    let loaded = Models::open(&root.0).unwrap();
    assert_eq!(loaded.credential_count().unwrap(), 1);
    assert_eq!(loaded.catalog(), catalog);
    assert!(!loaded.catalog().to_string().contains(key));
    root.bytes(&key_path, b"invalid\nheader");
    assert!(loaded.credential_count().is_err());
    assert!(Models::open(&root.0).is_err());
}

#[test]
fn assistant_projection_cannot_disclose_private_or_inactive_profiles() {
    let root = Fixture::new();
    let mut catalog = assistants();
    let mut private = catalog["profiles"][0].clone();
    private["id"] = json!("private");
    for snapshot in private["revisions"].as_array_mut().unwrap() {
        snapshot["id"] = json!("private");
        snapshot["access"]["published"] = json!(false);
    }
    catalog["profiles"].as_array_mut().unwrap().push(private);
    root.write("agent-profiles.json", &catalog);
    let loaded = Assistants::open(&root.0).unwrap();
    assert_eq!(loaded.list().len(), 2);
    assert!(loaded.detail("private").is_some());
    assert_eq!(loaded.public_profile("private"), Value::Null);
    assert_eq!(loaded.public_profile("unknown"), Value::Null);
    let public = loaded.default_public();
    assert_eq!(public["id"], "test");
    assert!(public.get("system_prompt").is_none());
    assert!(public.get("access").is_none());
    assert!(public.get("skill_ids").is_none());
    catalog["profiles"][1]["enabled"] = json!(false);
    for snapshot in catalog["profiles"][1]["revisions"].as_array_mut().unwrap() {
        snapshot["access"]["published"] = json!(true);
    }
    root.write("agent-profiles.json", &catalog);
    assert_eq!(
        Assistants::open(&root.0).unwrap().public_profile("private"),
        Value::Null
    );
}
