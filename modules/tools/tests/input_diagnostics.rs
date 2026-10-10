use exoharness_tools::{input_diagnostics, validate};
use serde_json::json;

#[test]
fn mixed_existing_reference_and_inline_source_reports_branch_constraints_without_echoing_code() {
    let schema = json!({"type":"object","oneOf":[{"required":["workspace_id"],"properties":{"files":{"maxProperties":0}}},{"required":["files"],"not":{"required":["workspace_id"]}}]});
    let input = json!({"workspace_id":"saved","files":{"main.py":"PRIVATE SOURCE".repeat(10000)}});
    let before = input.clone();
    let diagnostic = input_diagnostics(&schema, &input)
        .unwrap()
        .unwrap()
        .to_string();
    assert!(diagnostic.contains("/files"));
    assert!(diagnostic.contains("maxProperties"));
    assert!(!diagnostic.contains("PRIVATE SOURCE"));
    assert!(diagnostic.len() < 2048);
    assert_eq!(input, before);
    assert!(validate(&schema, &input).is_err());
    assert!(
        input_diagnostics(&schema, &json!({"workspace_id":"saved"}))
            .unwrap()
            .is_none()
    );
}

#[test]
fn missing_native_field_is_explicit_and_invalid_schema_is_not_correctable() {
    let schema = json!({"type":"object","required":["sections"]});
    let report = input_diagnostics(&schema, &json!({})).unwrap().unwrap();
    assert_eq!(report["errors"][0]["missing_field"], "sections");
    assert!(
        input_diagnostics(
            &json!({"$ref":"https://example.invalid/schema"}),
            &json!({})
        )
        .is_err()
    );
}
