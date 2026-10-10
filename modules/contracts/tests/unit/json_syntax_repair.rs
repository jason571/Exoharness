use super::*;
use serde_json::json;

#[test]
fn repairs_only_punctuation_and_reports_original_offsets() {
    for (source, expected) in [
        (
            r#"[{"content":{"text":"original"}}}]"#,
            json!([{"content":{"text":"original"}}]),
        ),
        (r#"{"a":[1,2,],}"#, json!({"a":[1,2]})),
        (
            r#"{"a":1 "b":[true false null]}"#,
            json!({"a":1,"b":[true,false,null]}),
        ),
        ("[[1] [2]]", json!([[1], [2]])),
        ("[{} } ]", json!([{}])),
        ("[]]", json!([])),
    ] {
        let fixed = repair_container(source).unwrap_or_else(|| panic!("{source}"));
        assert_eq!(fixed.value, expected);
        let mut replay = source.to_owned();
        for edit in fixed.edits.iter().rev() {
            assert_eq!(
                &source[edit.byte_offset..edit.byte_offset + edit.remove.len()],
                edit.remove
            );
            replay.replace_range(
                edit.byte_offset..edit.byte_offset + edit.remove.len(),
                &edit.insert,
            );
        }
        assert_eq!(replay, fixed.text);
        assert!(repair_container(&fixed.text).is_none());
    }
}

#[test]
fn preserves_strings_escapes_numbers_and_valid_json() {
    let original =
        r#"{"text":"中文 🐈 \\" } ] , literal","n":18446744073709551615,"v":[-2.5e3,false,null]}"#;
    // Construct with serde so escaped punctuation inside text is not syntax.
    let value = json!({"text":original,"n":u64::MAX,"v":[-2500.0,false,null]});
    let valid = value.to_string();
    assert!(repair_container(&valid).is_none());
    let malformed = format!("{valid}}}");
    let repaired = repair_container(&malformed).unwrap();
    assert_eq!(repaired.text, valid);
    assert_eq!(repaired.value, value);
    assert_eq!(repaired.edits[0].byte_offset, valid.len());
}

#[test]
fn refuses_ambiguity_truncation_duplicate_keys_and_non_json() {
    for source in [
        "[1,",
        "{\"a\":1",
        "[\"unterminated]",
        "[1,2}",
        "[1,,2]",
        "[,1]",
        "{\"a\" 1}",
        "[undefined]",
        "[NaN]",
        "[01,]",
        "[1. 2]",
        "[truefalse]",
        "{'a':1}",
        "```json\n[1,]\n```",
        "[1] trailing",
        "[1][2]",
        "[\"a\"\"b\"]",
        "{\"a\":[1}}",
        "{\"a\":1,\"a\":2,}",
        "{\"a\":1,\"\\u0061\":2,}",
        "{\"a\":{\"x\":1}},\"b\":2}",
        "[]}",
        "[1e9999,]",
    ] {
        assert!(repair_container(source).is_none(), "{source}");
    }
}

#[test]
fn bounds_work_and_never_drops_array_items() {
    assert!(repair_container(&format!("[\"{}\",]", "x".repeat(MAX_BYTES))).is_none());
    assert!(repair_container(&format!("[{}]", "[1,],".repeat(MAX_EDITS + 1))).is_none());
    assert!(
        repair_container(&format!(
            "{}1,{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        ))
        .is_none()
    );
    let source = format!(
        "[{}]",
        (0..64)
            .map(|i| format!(r#"{{"id":{i}}}"#))
            .collect::<Vec<_>>()
            .join(",")
    ) + "]";
    let result = repair_container(&source).unwrap();
    assert_eq!(result.value.as_array().unwrap().len(), 64);
    for i in 0..64 {
        assert_eq!(result.value[i]["id"], i);
    }
}
