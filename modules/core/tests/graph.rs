use exoharness_core::graph::{ExtensionSpec, GraphError, activation_order};

fn spec(id: &str, provides: &[&str], requires: &[&str]) -> ExtensionSpec {
    ExtensionSpec {
        id: id.into(),
        provides: provides.iter().map(|s| (*s).into()).collect(),
        requires: requires.iter().map(|s| (*s).into()).collect(),
    }
}

#[test]
fn resolves_dependencies_independently_of_registration_order() {
    let specs = vec![
        spec("agent", &["agent/v1"], &["journal/v1", "model/v1"]),
        spec("journal", &["journal/v1"], &[]),
        spec("model", &["model/v1"], &[]),
    ];
    assert_eq!(
        activation_order(&specs).unwrap(),
        ["journal", "model", "agent"]
    );
}

#[test]
fn missing_cycle_and_ambiguous_provider_fail_before_startup() {
    assert!(matches!(
        activation_order(&[spec("agent", &[], &["journal/v1"])]),
        Err(GraphError::MissingService { .. })
    ));
    assert!(matches!(
        activation_order(&[spec("a", &["a"], &["b"]), spec("b", &["b"], &["a"])]),
        Err(GraphError::Cycle(_))
    ));
    assert!(matches!(
        activation_order(&[spec("a", &["same"], &[]), spec("b", &["same"], &[])]),
        Err(GraphError::DuplicateProvider(_))
    ));
    assert!(matches!(
        activation_order(&[spec("a", &[], &[]), spec("a", &[], &[])]),
        Err(GraphError::DuplicateExtension(_))
    ));
    assert!(matches!(
        activation_order(&[spec("a", &["a"], &["a"])]),
        Err(GraphError::Cycle(_))
    ));
    assert!(matches!(
        activation_order(&[spec(" ", &[], &[])]),
        Err(GraphError::EmptyIdentifier)
    ));
}
