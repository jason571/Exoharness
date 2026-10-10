use exoharness_coding::{edits::*, paths::WorkspacePath};

fn edit(before: &str, after: &str) -> ExactEdit {
    ExactEdit {
        before: before.into(),
        after: after.into(),
    }
}

#[test]
fn rejects_absolute_traversal_windows_and_control_paths() {
    for path in [
        "", "/tmp/a", "../a", "a/../b", "a/./b", "a//b", "C:/a", "a\\b", "a\0b", "a\nb",
    ] {
        assert!(WorkspacePath::parse(path).is_err(), "{path:?}");
    }
    assert_eq!(
        WorkspacePath::parse("src/中文 file.ts").unwrap().as_str(),
        "src/中文 file.ts"
    );
}

#[test]
fn focused_unicode_edits_preserve_other_content() {
    let source = Document {
        revision: 4,
        text: "标题：旧\nbody unchanged\n".into(),
    };
    let result = apply_edits(&source, 4, &[edit("标题：旧", "标题：新")]).unwrap();
    assert_eq!(result.text, "标题：新\nbody unchanged\n");
    assert_eq!(result.revision, 5);
    assert_eq!(source.revision, 4);
}

#[test]
fn stale_ambiguous_overlapping_and_partial_edits_do_not_mutate_source() {
    let source = Document {
        revision: 1,
        text: "aaa 标题".into(),
    };
    assert_eq!(
        apply_edits(&source, 0, &[edit("标题", "新")]),
        Err(EditError::StaleRevision)
    );
    for edits in [
        vec![edit("aa", "b")],
        vec![edit("", "b")],
        vec![edit("标题", "新"), edit("missing", "x")],
    ] {
        assert_eq!(
            apply_edits(&source, 1, &edits),
            Err(EditError::AmbiguousAnchor)
        );
        assert_eq!(source.text, "aaa 标题");
    }
}

#[test]
fn no_op_keeps_revision_and_overflow_rejects_changes() {
    let source = Document {
        revision: u64::MAX,
        text: "text".into(),
    };
    assert_eq!(
        apply_edits(&source, u64::MAX, &[edit("text", "text")]).unwrap(),
        source
    );
    assert_eq!(
        apply_edits(&source, u64::MAX, &[edit("text", "changed")]),
        Err(EditError::RevisionOverflow)
    );
}
