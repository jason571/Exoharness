use exoharness_agent::{initial_skill_tools, project_request};
use exoharness_contracts::model::Tool;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn skill() -> JournalEntry {
    let text = "# Skill\nGLOBAL CULTURE AND DELIVERY REQUIREMENTS\n## Planning\nONLY PLANNING\n## Render\nKEEP RENDER REQUIREMENTS\n";
    let start = text.find("## Planning").unwrap();
    let end = text.find("## Render").unwrap();
    JournalEntry::ToolExchange {
        invocation_id: "read".into(),
        tool: "read_skill".into(),
        arguments: json!({"id":"skill"}),
        result: json!({"id":"skill","revision":2,"content":text,"_host_skill_context":{"version":1,"id":"skill","revision":2,"content_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"entry_tools":["prepare"],"workflow_tools":["prepare","render"],"stages":[{"tools":["prepare"],"omit_ranges":[]},{"tools":["render"],"omit_ranges":[[start,end]]}]}}),
    }
}
fn projection(entries: &[JournalEntry], names: &[&str]) -> String {
    let tools = names
        .iter()
        .map(|n| Tool {
            name: (*n).into(),
            description: String::new(),
            input_schema: json!({}),
        })
        .collect();
    serde_json::to_string(
        &project_request("system", entries, &BTreeSet::new(), tools, 1024).unwrap(),
    )
    .unwrap()
}
#[test]
fn declared_stage_keeps_global_rules_exact_pairs_and_immutable_original() {
    let entries = vec![JournalEntry::User("make a video".into()), skill()];
    let original = entries.clone();
    assert_eq!(initial_skill_tools(&entries).unwrap().0, vec!["prepare"]);
    let view = projection(&entries, &["render"]);
    assert!(!view.contains("ONLY PLANNING"));
    assert!(view.contains("GLOBAL CULTURE AND DELIVERY REQUIREMENTS"));
    assert!(view.contains("KEEP RENDER REQUIREMENTS"));
    assert!(view.contains("full_instructions_retained_in_journal"));
    assert_eq!(entries, original);
    assert!(projection(&entries, &["prepare"]).contains("ONLY PLANNING"));
    assert!(projection(&entries, &["unknown"]).contains("ONLY PLANNING"));
    assert!(projection(&entries, &["prepare", "render"]).contains("ONLY PLANNING"));
    let mut followup = entries.clone();
    followup.push(JournalEntry::User("change the plan".into()));
    assert!(projection(&followup, &["render"]).contains("ONLY PLANNING"));
    assert!(initial_skill_tools(&followup).is_none());
}
#[test]
fn invalid_hash_revision_range_ambiguity_and_unscoped_mcp_data_keep_full_skill() {
    for change in ["hash", "revision", "range", "overlap", "ambiguous", "tool"] {
        let mut s = skill();
        if let JournalEntry::ToolExchange { tool, result, .. } = &mut s {
            match change {
                "hash" => result["_host_skill_context"]["content_sha256"] = json!("invalid"),
                "revision" => result["_host_skill_context"]["revision"] = json!(1),
                "range" => {
                    result["_host_skill_context"]["stages"][1]["omit_ranges"] = json!([[1, 999999]])
                }
                "overlap" => {
                    result["_host_skill_context"]["stages"][1]["omit_ranges"] =
                        json!([[0, 10], [1, 12]])
                }
                "ambiguous" => {
                    let stage = result["_host_skill_context"]["stages"][1].clone();
                    result["_host_skill_context"]["stages"]
                        .as_array_mut()
                        .unwrap()
                        .push(stage);
                }
                "tool" => *tool = "mcp_untrusted".into(),
                _ => unreachable!(),
            }
        }
        assert!(
            projection(&[JournalEntry::User("create".into()), s], &["render"])
                .contains("ONLY PLANNING"),
            "{change}"
        );
    }
    let mut entries = vec![JournalEntry::User("create".into()), skill()];
    let unrelated = JournalEntry::ToolExchange {
        invocation_id: "other".into(),
        tool: "read_skill".into(),
        arguments: json!({"id":"other"}),
        result: Value::Null,
    };
    entries.push(unrelated);
    assert!(initial_skill_tools(&entries).is_none());
}
