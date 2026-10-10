use super::*;
#[test]
fn prefix_requires_a_complete_unambiguous_value() {
    for (text, expected, used) in [
        ("[1,2], broken", serde_json::json!([1, 2]), 5),
        ("\"key\": 1", serde_json::json!("key"), 5),
        ("12, 3", serde_json::json!(12), 2),
        ("true]", serde_json::json!(true), 4),
        ("null}", Value::Null, 4),
    ] {
        assert_eq!(parse_prefix(text).unwrap(), (expected, used));
    }
    for text in [
        "",
        "  ",
        "[1,",
        "{\"a\":",
        "\"partial",
        "{\"a\":1,\"a\":2}",
        "12x",
    ] {
        assert!(parse_prefix(text).is_err(), "{text}");
    }
}
#[test]
fn preserves_native_values_and_rejects_duplicate_keys_at_any_depth() {
    let text = r#"{"values":[null,false,18446744073709551615,-2,1.25,"中文\\\""],"object":{"title":"one"}}"#;
    assert_eq!(
        parse(text).unwrap(),
        serde_json::from_str::<Value>(text).unwrap()
    );
    for duplicate in [r#"{"a":1,"a":2}"#, r#"[{"a":1,"\u0061":2}]"#] {
        assert!(
            parse(duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicate object key")
        );
    }
    assert!(parse("{} trailing").is_err());
}
