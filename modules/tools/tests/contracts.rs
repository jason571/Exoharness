use exoharness_tools::{allowed, public_name, validate};
use serde_json::json;
#[test]
fn native_types_refs_and_constraints_are_checked_without_repairing_data() {
    let schema = json!({"type":"object","required":["sections"],"additionalProperties":false,"properties":{"sections":{"type":"array","minItems":1,"items":{"$ref":"#/$defs/section"}}},"$defs":{"section":{"type":"object","required":["duration"],"properties":{"duration":{"type":"integer","minimum":1,"maximum":30}},"additionalProperties":false}}});
    assert!(validate(&schema, &json!({"sections":[{"duration":30}]})).is_ok());
    for bad in [
        json!({"sections":"[]"}),
        json!({"sections":[]}),
        json!({"sections":[{"duration":31}]}),
        json!({"sections":[{"duration":3,"invented":true}]}),
    ] {
        assert!(validate(&schema, &bad).is_err());
    }
    assert!(
        validate(
            &json!({"$ref":"https://example.invalid/schema"}),
            &json!({})
        )
        .is_err()
    );
}
#[test]
fn names_are_stable_bounded_and_never_alias_raw_names_or_providers() {
    let name = public_name("server-one", "汉字 tool/name");
    assert!(name.len() <= 64);
    assert_eq!(name, public_name("server-one", "汉字 tool/name"));
    assert_ne!(name, public_name("server-two", "汉字 tool/name"));
    assert_ne!(public_name("s", "a-b"), public_name("s", "a_b"));
    assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
}
#[test]
fn skill_selection_never_grants_an_execution_tool_or_mcp() {
    let profile = json!({"access":{"skills":{"mode":"all"},"tools":{"mode":"selected","ids":["tool:native:sandbox"]},"mcp":{"mode":"none"}}});
    assert!(allowed(&profile, "skills", "skill:global:_:anything"));
    assert!(allowed(&profile, "tools", "tool:native:sandbox"));
    assert!(!allowed(&profile, "tools", "arbitrary"));
    assert!(!allowed(&profile, "mcp", "mcp_server:global:_:anything"));
    assert!(!allowed(&json!({}), "tools", "tool:native:sandbox"));
}
