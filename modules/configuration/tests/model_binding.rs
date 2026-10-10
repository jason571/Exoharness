#[path = "support/catalogs.rs"]
#[allow(dead_code)]
mod support;
use exoharness_configuration::Models;
use support::{Fixture, model};

#[test]
fn binding_retains_exact_model_revision_and_secret_across_rotation_and_disable() {
    let root = Fixture::new();
    let models = Models::initialize(&root.0).unwrap();
    let value = model();
    let provider = value["providers"][0].clone();
    let entry = value["models"][0].clone();
    models
        .save_configuration(provider.clone(), entry.clone(), Some("first-key"), None)
        .unwrap();
    let previous = models.bind_enabled("测试模型").unwrap();
    models
        .save_configuration(provider, entry, Some("second-key"), None)
        .unwrap();
    let current = models.bind_enabled("测试模型").unwrap();
    assert_eq!(previous.revision, 1);
    assert_eq!(current.revision, 2);
    previous.with_credential(|key| assert_eq!(key, Some("first-key")));
    current.with_credential(|key| assert_eq!(key, Some("second-key")));
    models.set_provider_enabled("api", false, None).unwrap();
    assert!(models.bind_enabled("测试模型").is_err());
    assert!(models.bind_enabled("missing").is_err());
    previous.with_credential(|key| assert_eq!(key, Some("first-key")));
}
