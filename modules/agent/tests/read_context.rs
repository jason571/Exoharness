use exoharness_agent::project_request;
use exoharness_contracts::model::Message;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn append_read(
    entries: &mut Vec<JournalEntry>,
    id: &str,
    path: &str,
    digest: &str,
    offset: usize,
    text: &str,
    next: Value,
) {
    let body = json!({"workspace_id":"owned","revision":4,"file":{"path":path,"sha256":digest},"offset":offset,"next_offset":next,"content":text,"_durable_read_body":true});
    entries.push(JournalEntry::ToolExchange {
        invocation_id: id.into(),
        tool: "read".into(),
        arguments: json!({"workspace_id":"owned","path":path,"offset":offset}),
        result: body.clone(),
    });
    entries.push(JournalEntry::Checkpoint {
        invocation_id: id.into(),
        reference: format!("store:{id}"),
        summary: body,
    });
}
fn bodies(entries: &[JournalEntry]) -> Vec<Value> {
    project_request("system", entries, &BTreeSet::new(), vec![], 200)
        .unwrap()
        .messages
        .into_iter()
        .filter_map(|m| match m {
            Message::ToolResult { content, .. } => {
                let v: Value = serde_json::from_str(&content).unwrap();
                let r = v.get("result").unwrap_or(&v);
                (r["_durable_read_body"] == true).then(|| r.clone())
            }
            _ => None,
        })
        .collect()
}
#[test]
fn three_authored_files_keep_all_exact_read_bodies_instead_of_rotating_two_slots() {
    let mut entries = vec![
        JournalEntry::ToolExchange {
            invocation_id: "write".into(),
            tool: "write".into(),
            arguments: json!({"files":{"index.html":"html","script.js":"js","check.py":"check"}}),
            result: json!({"workspace_id":"owned"}),
        },
        JournalEntry::Checkpoint {
            invocation_id: "write".into(),
            reference: "store:write".into(),
            summary: json!({"workspace_id":"owned"}),
        },
    ];
    for (id, path, body) in [
        ("a", "index.html", "html"),
        ("b", "script.js", "js"),
        ("c", "check.py", "check"),
    ] {
        append_read(
            &mut entries,
            id,
            path,
            &"a".repeat(64),
            0,
            body,
            Value::Null,
        );
    }
    let original = entries.clone();
    let result = bodies(&entries);
    assert_eq!(
        result.iter().filter(|r| r.get("content").is_some()).count(),
        3
    );
    assert_eq!(entries, original);
}
#[test]
fn exact_duplicate_and_covered_partial_read_compact_to_one_complete_body() {
    let mut entries = vec![];
    append_read(
        &mut entries,
        "partial",
        "file.txt",
        &"a".repeat(64),
        2,
        "中文",
        Value::Null,
    );
    append_read(
        &mut entries,
        "full",
        "file.txt",
        &"a".repeat(64),
        0,
        "AB中文",
        Value::Null,
    );
    append_read(
        &mut entries,
        "again",
        "file.txt",
        &"a".repeat(64),
        0,
        "AB中文",
        Value::Null,
    );
    let result = bodies(&entries);
    assert_eq!(
        result.iter().filter(|r| r.get("content").is_some()).count(),
        1
    );
    assert_eq!(result[2]["content"], "AB中文");
    assert!(result[0]["read_body_reference"].is_string());
}
#[test]
fn distinct_pages_versions_workspaces_and_unverified_reads_never_lose_unique_text() {
    let mut entries = vec![];
    append_read(
        &mut entries,
        "p0",
        "file.txt",
        &"a".repeat(64),
        0,
        "page-zero",
        json!(9),
    );
    append_read(
        &mut entries,
        "p1",
        "file.txt",
        &"a".repeat(64),
        9,
        "page-one",
        Value::Null,
    );
    append_read(
        &mut entries,
        "v2",
        "file.txt",
        &"b".repeat(64),
        0,
        "new-version",
        Value::Null,
    );
    entries.push(JournalEntry::ToolExchange {
        invocation_id: "external".into(),
        tool: "external".into(),
        arguments: json!({}),
        result: json!({"content":"private upload","_durable_read_body":true}),
    });
    let result = bodies(&entries);
    assert_eq!(
        result.iter().filter(|r| r.get("content").is_some()).count(),
        4
    );
}
#[test]
fn focused_new_read_keeps_its_excerpt_and_the_prior_complete_source() {
    let mut entries = vec![];
    append_read(
        &mut entries,
        "full",
        "file.txt",
        &"a".repeat(64),
        0,
        "AB中文",
        Value::Null,
    );
    append_read(
        &mut entries,
        "focus",
        "file.txt",
        &"a".repeat(64),
        2,
        "中文",
        Value::Null,
    );
    let result = bodies(&entries);
    assert_eq!(result[0]["content"], "AB中文");
    assert_eq!(result[1]["content"], "中文");
}

#[test]
fn overlapping_pages_and_different_workspaces_do_not_compact_each_other() {
    let mut entries = vec![];
    append_read(
        &mut entries,
        "left",
        "file.txt",
        &"a".repeat(64),
        0,
        "ABCDE",
        json!(5),
    );
    append_read(
        &mut entries,
        "right",
        "file.txt",
        &"a".repeat(64),
        3,
        "DEFGH",
        Value::Null,
    );
    append_read(
        &mut entries,
        "other",
        "file.txt",
        &"a".repeat(64),
        0,
        "ABCDE",
        json!(5),
    );
    for entry in &mut entries {
        match entry {
            JournalEntry::Checkpoint {
                invocation_id,
                summary,
                ..
            } if invocation_id == "other" => {
                summary["workspace_id"] = json!("different-owner-workspace")
            }
            JournalEntry::ToolExchange {
                invocation_id,
                result,
                ..
            } if invocation_id == "other" => {
                result["workspace_id"] = json!("different-owner-workspace")
            }
            _ => {}
        }
    }
    assert_eq!(
        bodies(&entries)
            .iter()
            .filter(|r| r.get("content").is_some())
            .count(),
        3
    );
}
#[test]
fn inconsistent_body_or_incomplete_metadata_is_never_silently_discarded() {
    let mut entries = vec![];
    append_read(
        &mut entries,
        "before",
        "file.txt",
        &"a".repeat(64),
        0,
        "ABC",
        Value::Null,
    );
    append_read(
        &mut entries,
        "after",
        "file.txt",
        &"a".repeat(64),
        0,
        "ABD",
        Value::Null,
    );
    append_read(
        &mut entries,
        "bad-digest",
        "file.txt",
        "incomplete",
        0,
        "ABD",
        Value::Null,
    );
    assert_eq!(
        bodies(&entries)
            .iter()
            .filter(|r| r.get("content").is_some())
            .count(),
        3
    );
}

fn native_write(
    entries: &mut Vec<JournalEntry>,
    id: &str,
    text: &str,
    revision: u64,
    valid: bool,
) -> String {
    let digest = if valid {
        format!("{:x}", Sha256::digest(text.as_bytes()))
    } else {
        "f".repeat(64)
    };
    let summary = json!({"workspace_id":"owned","revision":revision,"_host_execution":false,"files":[{"path":"file.txt","sha256":digest}]});
    entries.push(JournalEntry::ToolExchange {
        invocation_id: id.into(),
        tool: "write".into(),
        arguments: json!({"files":{"file.txt":text}}),
        result: summary.clone(),
    });
    entries.push(JournalEntry::Checkpoint {
        invocation_id: id.into(),
        reference: format!("store:{id}"),
        summary,
    });
    digest
}
#[test]
fn obsolete_read_is_replaced_with_exact_attested_current_authoring_without_rewriting_history() {
    let mut entries = vec![JournalEntry::User("Fix the source".into())];
    let old = "OLD-".repeat(800);
    let new = "NEW-中文-".repeat(800);
    let old_hash = native_write(&mut entries, "old-write", &old, 1, true);
    append_read(
        &mut entries,
        "old-read",
        "file.txt",
        &old_hash,
        0,
        &old,
        Value::Null,
    );
    native_write(&mut entries, "new-write", &new, 5, true);
    let original = entries.clone();
    let request = project_request("system", &entries, &BTreeSet::new(), vec![], 200).unwrap();
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(!serialized.contains(&old));
    assert!(serialized.contains(&new));
    assert!(serialized.contains("current_authored_sources"));
    assert!(serialized.contains("read_body_superseded_by"));
    assert_eq!(entries, original);
}
#[test]
fn current_full_read_is_kept_without_an_extra_authored_source_copy() {
    let mut entries = vec![];
    let old = "OLD-".repeat(800);
    let new = "NEW-".repeat(800);
    let old_hash = native_write(&mut entries, "old-write", &old, 1, true);
    append_read(
        &mut entries,
        "old-read",
        "file.txt",
        &old_hash,
        0,
        &old,
        Value::Null,
    );
    let new_hash = native_write(&mut entries, "new-write", &new, 5, true);
    append_read(
        &mut entries,
        "new-read",
        "file.txt",
        &new_hash,
        0,
        &new,
        Value::Null,
    );
    let request = serde_json::to_string(
        &project_request("system", &entries, &BTreeSet::new(), vec![], 200).unwrap(),
    )
    .unwrap();
    assert!(!request.contains(&old));
    assert!(request.contains(&new));
    assert!(!request.contains("current_authored_sources"));
}
#[test]
fn mismatched_hash_unknown_failure_and_new_user_scope_never_fabricate_current_source() {
    for variant in ["hash", "failure", "new-user"] {
        let mut entries = vec![JournalEntry::User("original".into())];
        let old = "OLD-".repeat(800);
        let new = "NEW-".repeat(800);
        let old_hash = native_write(&mut entries, "old-write", &old, 1, true);
        append_read(
            &mut entries,
            "old-read",
            "file.txt",
            &old_hash,
            0,
            &old,
            Value::Null,
        );
        native_write(&mut entries, "new-write", &new, 5, variant != "hash");
        if variant == "failure" {
            entries.push(JournalEntry::ToolExchange {
                invocation_id: "failed".into(),
                tool: "run".into(),
                arguments: json!({}),
                result: json!({"workspace_id":"owned","_host_sandbox_failure":1}),
            });
        }
        if variant == "new-user" {
            entries.push(JournalEntry::User("Compare old and new".into()));
        }
        let request = serde_json::to_string(
            &project_request("system", &entries, &BTreeSet::new(), vec![], 200).unwrap(),
        )
        .unwrap();
        assert!(request.contains(&old), "{variant}");
        assert!(!request.contains("current_authored_sources"), "{variant}");
    }
}

#[test]
fn large_current_file_and_previous_user_version_keep_reference_access_without_automatic_full_injection()
 {
    for new_turn in [false, true] {
        let mut entries = vec![JournalEntry::User("old task".into())];
        let old = "OLD-".repeat(800);
        let old_hash = native_write(&mut entries, "old-write", &old, 1, true);
        append_read(
            &mut entries,
            "old-read",
            "file.txt",
            &old_hash,
            0,
            &old,
            Value::Null,
        );
        if new_turn {
            entries.push(JournalEntry::User(
                "Compare this old version with my new code".into(),
            ));
        }
        native_write(
            &mut entries,
            "new-write",
            &"NEW-".repeat(if new_turn { 800 } else { 5000 }),
            5,
            true,
        );
        let request = serde_json::to_string(
            &project_request("system", &entries, &BTreeSet::new(), vec![], 200).unwrap(),
        )
        .unwrap();
        assert!(request.contains(&old));
        assert!(!request.contains("current_authored_sources"));
    }
}

#[test]
fn truncated_or_deleted_latest_inventory_does_not_claim_an_old_hash_is_current() {
    for truncated in [false, true] {
        let mut entries = vec![];
        let old = "OLD-".repeat(800);
        let new = "NEW-".repeat(800);
        let hash = native_write(&mut entries, "old", &old, 1, true);
        append_read(
            &mut entries,
            "read",
            "file.txt",
            &hash,
            0,
            &old,
            Value::Null,
        );
        native_write(&mut entries, "new", &new, 5, true);
        let summary = json!({"workspace_id":"owned","revision":6,"_host_execution":false,"files":[],"file_inventory_truncated":truncated});
        entries.push(JournalEntry::ToolExchange {
            invocation_id: "partial-inventory".into(),
            tool: "inventory".into(),
            arguments: json!({}),
            result: summary.clone(),
        });
        entries.push(JournalEntry::Checkpoint {
            invocation_id: "partial-inventory".into(),
            reference: "store:partial".into(),
            summary,
        });
        let request = serde_json::to_string(
            &project_request("system", &entries, &BTreeSet::new(), vec![], 200).unwrap(),
        )
        .unwrap();
        assert!(request.contains(&old));
        assert!(!request.contains("current_authored_sources"));
    }
}
