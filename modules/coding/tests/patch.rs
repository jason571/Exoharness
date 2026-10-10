use exoharness_coding::patch::{parse, prepare};
use std::collections::BTreeMap;
#[test]
fn atomic_unicode_multifile_add_update_move_delete_preserves_other_sources() {
    let sources = BTreeMap::from([
        ("a.txt".into(), "标题\r\n旧内容\r\n".into()),
        ("old.txt".into(), "delete\n".into()),
        ("keep.txt".into(), "untouched".into()),
    ]);
    let original = sources.clone();
    let patch = "*** Begin Patch\n*** Update File: a.txt\n*** Move to: docs/a.txt\n@@\n 标题\n-旧内容\n+新内容\n*** Delete File: old.txt\n*** Add File: src/main.js\n+console.log('中文');\n*** End Patch";
    let result = prepare(&parse(patch).unwrap(), &sources).unwrap();
    assert_eq!(result.writes["docs/a.txt"], "标题\r\n新内容\r\n");
    assert_eq!(result.writes["src/main.js"], "console.log('中文');\n");
    assert_eq!(result.deletes, vec!["a.txt", "old.txt"]);
    assert_eq!(sources, original);
    assert!(!result.writes.contains_key("keep.txt"));
}
#[test]
fn ambiguous_missing_partial_conflicting_and_overwrite_patches_fail_closed() {
    let sources = BTreeMap::from([("a".into(), "repeat\nrepeat\n".into())]);
    for patch in [
        "*** Begin Patch\n*** Update File: a\n@@\n-repeat\n+new\n*** End Patch",
        "*** Begin Patch\n*** Add File: b\n+new\n*** Update File: a\n@@\n-missing\n+bad\n*** End Patch",
        "*** Begin Patch\n*** Add File: a\n+new\n*** End Patch",
        "*** Begin Patch\n*** Add File: b\n+one\n*** Add File: b\n+two\n*** End Patch",
        "*** Begin Patch\n*** Update File: a\n*** Move to: a\n*** End Patch",
    ] {
        assert!(prepare(&parse(patch).unwrap(), &sources).is_err());
    }
}
#[test]
fn exact_context_and_eof_disambiguate_without_fuzzy_matching() {
    let sources = BTreeMap::from([("a".into(), "block one\nsame\nblock two\nsame".into())]);
    let patch = "*** Begin Patch\n*** Update File: a\n@@ block two\n-same\n+changed\n*** End of File\n*** End Patch";
    assert_eq!(
        prepare(&parse(patch).unwrap(), &sources).unwrap().writes["a"],
        "block one\nsame\nblock two\nchanged"
    );
}
#[test]
fn malformed_binary_path_traversal_readonly_and_empty_hunks_are_rejected() {
    for patch in [
        "garbage",
        "*** Begin Patch\n*** Add File: ../outside\n+bad\n*** End Patch",
        "*** Begin Patch\n*** Add File: /host\n+bad\n*** End Patch",
        "*** Begin Patch\n*** Add File: skill/main.py\n+bad\n*** End Patch",
        "*** Begin Patch\n*** Update File: a\n@@\n+no context\n*** End Patch",
        "*** Begin Patch\n*** Update File: a\n@@\nnot marked\n*** End Patch",
        "*** Begin Patch\n*** End Patch",
    ] {
        assert!(parse(patch).is_err(), "{patch}");
    }
}
