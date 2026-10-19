use super::sandbox_tool;
use crate::validate;
use serde_json::json;

#[test]
fn skill_read_accepts_optional_bounded_typed_loading_and_preserves_legacy_calls() {
    let schema = super::base()
        .into_iter()
        .find(|t| t.name == "read_skill")
        .unwrap()
        .input_schema;
    for args in [
        json!({"id":"skill"}),
        json!({"id":"skill","path":"SKILL.md","load_tools":[]}),
        json!({"id":"skill","load_tools":["sandbox_run","workspace_read"]}),
    ] {
        assert!(validate(&schema, &args).is_ok());
    }
    for args in [
        json!({"id":"skill","load_tools":"sandbox_run"}),
        json!({"id":"skill","load_tools":[""]}),
        json!({"id":"skill","load_tools":["sandbox_run","sandbox_run"]}),
        json!({"id":"skill","load_tools":(0..17).map(|n|format!("tool{n}")).collect::<Vec<_>>()}),
    ] {
        assert!(validate(&schema, &args).is_err());
    }
}

#[test]
fn discovery_accepts_scopes_exact_batches_and_bound_cursors_without_widening_schemas() {
    let schema = super::base()
        .into_iter()
        .find(|t| t.name == "search_capabilities")
        .unwrap()
        .input_schema;
    for args in [
        json!({"query":"lookup","offset":12}),
        json!({"query":"","kind":"capability"}),
        json!({"query":"","kind":"tool","capability_id":"authorized","names":["one","two"]}),
        json!({"query":"lookup","kind":"tool","cursor":format!("{}:12","a".repeat(64))}),
    ] {
        assert!(validate(&schema, &args).is_ok());
    }
    for args in [
        json!({"query":"","names":"one"}),
        json!({"query":"","names":[]}),
        json!({"query":"","names":["one","one"]}),
        json!({"query":"","capability_id":""}),
        json!({"query":"","kind":"unknown"}),
        json!({"query":"","names":(0..17).map(|n| format!("n{n}")).collect::<Vec<_>>()}),
    ] {
        assert!(validate(&schema, &args).is_err());
    }
}

#[test]
fn activation_schema_preserves_old_calls_and_accepts_only_explicit_valid_modes() {
    let schema = super::base()
        .into_iter()
        .find(|t| t.name == "activate_tools")
        .unwrap()
        .input_schema;
    for call in [
        json!({"names":["known"]}),
        json!({"names":["known"],"mode":"add"}),
        json!({"names":[],"mode":"replace"}),
    ] {
        assert!(validate(&schema, &call).is_ok());
    }
    for call in [
        json!({"names":["known"],"mode":"unknown"}),
        json!({"names":"known"}),
        json!({"query":"known"}),
        json!({"names":["known","known"]}),
    ] {
        assert!(validate(&schema, &call).is_err());
    }
}

#[test]
fn sandbox_modes_are_explicit_and_do_not_allow_implicit_source_overwrite() {
    let schema = sandbox_tool().input_schema;
    let existing = json!({"runtime":"python","entrypoint":"check.py","output_files":[],"workspace_id":"a".repeat(64),"expected_revision":2});
    assert!(validate(&schema, &existing).is_ok());
    let mut explicit_empty = existing.clone();
    explicit_empty["files"] = json!({});
    assert!(validate(&schema, &explicit_empty).is_ok());
    let new = json!({"runtime":"python","entrypoint":"main.py","output_files":[],"files":{"main.py":"print(1)"}});
    assert!(validate(&schema, &new).is_ok());
    for field in ["files", "skill_id"] {
        let mut invalid = existing.clone();
        invalid[field] = if field == "files" {
            json!({"check.py":"print(1)"})
        } else {
            json!("package")
        };
        assert!(validate(&schema, &invalid).is_err());
    }
    let mut missing_revision = existing.clone();
    missing_revision
        .as_object_mut()
        .unwrap()
        .remove("expected_revision");
    assert!(validate(&schema, &missing_revision).is_err());
    let mut empty = new.clone();
    empty["files"] = json!({});
    assert!(validate(&schema, &empty).is_err());
    let mut orphan_revision = new.clone();
    orphan_revision["expected_revision"] = json!(2);
    assert!(validate(&schema, &orphan_revision).is_err());
    let mut package = new;
    package["skill_id"] = json!("authorized-package");
    assert!(validate(&schema, &package).is_ok());
}
