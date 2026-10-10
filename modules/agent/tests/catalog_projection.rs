use exoharness_agent::project_request;
use exoharness_contracts::model::Message;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn exchange(id: &str, tool: &str, arguments: Value, result: Value) -> JournalEntry {
    JournalEntry::ToolExchange {
        invocation_id: id.into(),
        tool: tool.into(),
        arguments,
        result,
    }
}
fn discovery() -> JournalEntry {
    exchange(
        "query",
        "search_capabilities",
        json!({"query":"render","kind":"tool"}),
        json!({
            "items":[
                {"kind":"tool","name":"renderer","capability_id":"provider","description":"LONG TOOL DESCRIPTION ".repeat(5000)},
                {"kind":"tool","name":"other-tool","description":"OTHER TOOL DESCRIPTION"},
                {"kind":"skill","id":"unread-skill","revision":3,"description":"KEEP UNREAD SKILL"}
            ],"related_skills":[{"id":"another-skill","description":"KEEP RELATED SKILL"}],
            "total":20,"next_offset":12,"note":"KEEP DISCOVERY GUIDANCE"
        }),
    )
}
fn activation(error: bool) -> JournalEntry {
    exchange(
        "load",
        "activate_tools",
        json!({"names":["renderer"]}),
        json!({"loaded":["renderer"],"executed":false,"isError":error}),
    )
}
fn project(entries: &[JournalEntry]) -> Vec<Message> {
    project_request("system", entries, &BTreeSet::new(), vec![], 2048)
        .unwrap()
        .messages
}
#[test]
fn selected_discovery_becomes_small_atomic_receipt_without_changing_journal_or_data() {
    let mut entries = vec![JournalEntry::User("produce output".into()), discovery()];
    let before = serde_json::to_string(&entries).unwrap().len();
    assert!(serde_json::to_string(&project(&entries)).unwrap().len() < before / 10);
    entries.extend([
        activation(false),
        exchange(
            "data",
            "renderer",
            json!({"prompt":"EXACT STRUCTURED INPUT"}),
            json!({"result":"EXACT BUSINESS OUTPUT","url":"https://media.example/final"}),
        ),
    ]);
    let original = entries.clone();
    let messages = project(&entries);
    let text = serde_json::to_string(&messages).unwrap();
    assert!(text.len() < before / 10, "{} vs {before}", text.len());
    assert!(!text.contains("LONG TOOL DESCRIPTION"));
    for value in [
        "renderer",
        "EXACT STRUCTURED INPUT",
        "EXACT BUSINESS OUTPUT",
    ] {
        assert!(text.contains(value), "{value}");
    }
    let receipt = messages
        .iter()
        .find_map(|m| match m {
            Message::ToolResult {
                call_id, content, ..
            } if call_id == "query" => Some(serde_json::from_str::<Value>(content).unwrap()),
            _ => None,
        })
        .unwrap();
    assert_eq!(receipt["catalog_selection_retired"], true);
    assert_eq!(receipt["items"].as_array().unwrap().len(), 1);
    assert!(!text.contains("other-tool"));
    assert!(!text.contains("KEEP UNREAD SKILL"));
    for (index, message) in messages.iter().enumerate() {
        if let Message::Assistant { calls, .. } = message
            && !calls.is_empty()
        {
            assert!(
                matches!(&messages[index+1], Message::ToolResult { call_id, .. } if call_id == &calls[0].id)
            );
        }
    }
    assert_eq!(entries, original);
}
#[test]
fn no_selection_failed_selection_unrelated_selection_and_new_turn_keep_discovery_exact() {
    for suffix in [
        vec![],
        vec![activation(true)],
        vec![
            JournalEntry::User("different task".into()),
            activation(false),
        ],
    ] {
        let mut entries = vec![JournalEntry::User("create".into()), discovery()];
        entries.extend(suffix);
        assert!(
            serde_json::to_string(&project(&entries))
                .unwrap()
                .contains("LONG TOOL DESCRIPTION")
        );
    }
}
#[test]
fn malformed_or_failed_search_results_are_not_reinterpreted() {
    for result in [
        json!({"items":"encoded or unexpected data"}),
        json!({"isError":true,"items":[{"kind":"tool","name":"renderer","description":"KEEP ERROR DATA"}]}),
    ] {
        let entries = vec![
            JournalEntry::User("create".into()),
            exchange("query", "search_capabilities", json!({}), result.clone()),
            activation(false),
        ];
        let receipt = project(&entries)
            .into_iter()
            .find_map(|m| match m {
                Message::ToolResult {
                    call_id, content, ..
                } if call_id == "query" => Some(serde_json::from_str::<Value>(&content).unwrap()),
                _ => None,
            })
            .unwrap();
        assert_eq!(receipt, result);
    }
}

#[test]
fn unresolved_directory_uses_unicode_summaries_but_authoring_and_mcp_results_remain_exact() {
    let description = "风格检索与精确调用说明。".repeat(1000);
    let narration = "完整旁白与四镜分镜".repeat(1000);
    let result = json!({"items":[{"kind":"tool","name":"exact-name","description":description,"schema_loaded":false}],"total":17,"next_offset":12});
    let entries = vec![
        JournalEntry::User("generate".into()),
        exchange(
            "search",
            "search_capabilities",
            json!({"query":"generate"}),
            result.clone(),
        ),
        exchange(
            "author",
            "arbitrary_mcp",
            json!({"storyboard":[narration.clone()]}),
            json!({"description":description,"narration":narration}),
        ),
    ];
    let original = entries.clone();
    let messages = project(&entries);
    let lookup: Value = messages
        .iter()
        .find_map(|m| match m {
            Message::ToolResult {
                call_id, content, ..
            } if call_id == "search" => Some(serde_json::from_str(content).unwrap()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        lookup["items"][0]["description"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        160
    );
    assert_eq!(lookup["items"][0]["description_is_summary"], true);
    assert_eq!(lookup["items"][0]["name"], "exact-name");
    assert_eq!(lookup["total"], 17);
    assert_eq!(lookup["next_offset"], 12);
    let authored = messages
        .iter()
        .find_map(|m| match m {
            Message::Assistant { calls, .. } => calls.iter().find(|c| c.id == "author"),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        authored.arguments,
        json!({"storyboard":[narration.clone()]})
    );
    let external: Value = messages
        .iter()
        .find_map(|m| match m {
            Message::ToolResult {
                call_id, content, ..
            } if call_id == "author" => Some(serde_json::from_str(content).unwrap()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        external,
        json!({"description":description,"narration":narration})
    );
    assert_eq!(entries, original);
}

#[test]
fn loaded_skill_description_is_retired_only_for_exact_id_revision_and_root_instructions() {
    for (path, error, revision, retired) in [
        ("SKILL.md", false, 3, true),
        ("references/example.md", false, 3, false),
        ("SKILL.md", true, 3, false),
        ("SKILL.md", false, 4, false),
    ] {
        let entries = vec![
            JournalEntry::User("work".into()),
            exchange(
                "search",
                "search_capabilities",
                json!({"query":"skill"}),
                json!({"items":[],"related_skills":[{"id":"skill","revision":3,"description":"skill recognition metadata"}]}),
            ),
            exchange(
                "read",
                "read_skill",
                json!({"id":"skill","path":path}),
                json!({"id":"skill","revision":revision,"content":"KEEP EVERY INSTRUCTION AND STORYBOARD RULE","isError":error}),
            ),
        ];
        let messages = project(&entries);
        let lookup: Value = messages
            .iter()
            .find_map(|m| match m {
                Message::ToolResult {
                    call_id, content, ..
                } if call_id == "search" => Some(serde_json::from_str(content).unwrap()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            lookup["related_skills"][0].get("description").is_none(),
            retired
        );
        assert!(
            serde_json::to_string(&messages)
                .unwrap()
                .contains("KEEP EVERY INSTRUCTION AND STORYBOARD RULE")
        );
    }
}

#[test]
fn completed_selection_retires_all_obsolete_pages_but_keeps_later_discovery_and_exact_business_json()
 {
    let mut entries = vec![JournalEntry::User("create requested output".into())];
    for page in 0..9 {
        entries.push(exchange(&format!("page-{page}"), "search_capabilities", json!({"query":"old","offset":page*12}),
            json!({"items":[{"kind":"tool","name":format!("unrelated-{page}"),"description":"OLD DIRECTORY ".repeat(200)}, {"kind":"tool","name":"renderer","description":"SELECTED DIRECTORY"}],"next_offset":page*12+12})));
    }
    entries.push(activation(false));
    entries.push(exchange("later", "search_capabilities", json!({"query":"future"}),
        json!({"items":[{"kind":"tool","name":"future-operation","description":"KEEP NEW SEARCH"}]})));
    entries.push(exchange(
        "business",
        "renderer",
        json!({"storyboard":["DO NOT COMPRESS SHOTS".repeat(200)]}),
        json!({"result":"DO NOT COMPRESS BUSINESS RESULT".repeat(200)}),
    ));
    let original = entries.clone();
    let text = serde_json::to_string(&project(&entries)).unwrap();
    assert!(!text.contains("OLD DIRECTORY"));
    assert!(!text.contains("unrelated-"));
    assert!(text.contains("KEEP NEW SEARCH"));
    assert!(text.contains("DO NOT COMPRESS SHOTS"));
    assert!(text.contains("DO NOT COMPRESS BUSINESS RESULT"));
    assert!(text.len() < serde_json::to_string(&entries).unwrap().len() / 2);
    assert_eq!(entries, original);
}
