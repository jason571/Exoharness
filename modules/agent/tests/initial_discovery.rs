use exoharness_agent::{EngineState, InitialDiscovery};
use exoharness_contracts::model::Tool;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};

fn index() -> Value {
    json!({"items":[{"kind":"capability","id":"provider","name":"Renderer","revision":7,"tool_count":3,
        "description":"Service recognition metadata. ".repeat(100),
        "discover":{"tool":"search_capabilities","arguments":{"query":"","kind":"tool","capability_id":"provider"}}}],
        "next_page":{"query":"","kind":"capability","cursor":"exact-cursor"},"total":13})
}
fn skills() -> Vec<Value> {
    vec![
        json!({"id":"selected","name":"Selected","revision":3,"description":"SELECTED RECOGNITION"}),
        json!({"id":"unread","name":"Unread","revision":2,"description":"KEEP UNREAD RECOGNITION"}),
    ]
}
fn tools() -> Vec<Tool> {
    vec![Tool {
        name: "render".into(),
        description: "KEEP LIVE DESCRIPTION".into(),
        input_schema: json!({"type":"object","required":["exact"],"properties":{"exact":{"type":"integer"}}}),
    }]
}
fn exchange(tool: &str, arguments: Value, result: Value) -> JournalEntry {
    JournalEntry::ToolExchange {
        invocation_id: format!("call-{tool}"),
        tool: tool.into(),
        arguments,
        result,
    }
}
fn activation(failed: bool) -> JournalEntry {
    exchange(
        "activate_tools",
        json!({"names":["render"]}),
        json!({"loaded":["render"],"executed":false,"isError":failed}),
    )
}
fn root(path: &str, revision: u64) -> JournalEntry {
    exchange(
        "read_skill",
        json!({"id":"selected","path":path}),
        json!({"id":"selected","revision":revision,"content":"EXACT ROOT INSTRUCTIONS"}),
    )
}
fn full(system: &str) -> String {
    format!(
        "{system}\nAuthorized capability index (service summaries only; no tool schemas; use next_page unchanged for additional services): {}\nAuthorized Skill candidates for this task (metadata only; use read_skill for applicable instructions, not all candidates): {}",
        index(),
        json!(skills())
    )
}
#[test]
fn unresolved_initial_request_is_byte_identical_to_previous_system() {
    let discovery = InitialDiscovery::new(index(), skills());
    assert_eq!(
        discovery.system(
            "identity and policy",
            &[JournalEntry::User("work".into())],
            &[]
        ),
        full("identity and policy")
    );
}
#[test]
fn active_selection_defers_catalog_and_keeps_exact_rediscovery_navigation() {
    let index = index();
    let discovery = InitialDiscovery::new(index.clone(), skills());
    let entries = vec![JournalEntry::User("work".into()), activation(false)];
    let before = entries.clone();
    let original_tools = tools();
    let system = discovery.system("DO NOT CHANGE THIS POLICY", &entries, &original_tools);
    assert!(system.starts_with("DO NOT CHANGE THIS POLICY\n"));
    assert!(!system.contains("Service recognition metadata."));
    for expected in [
        "exact-cursor",
        "catalog_retrieval",
        "KEEP UNREAD RECOGNITION",
        "SELECTED RECOGNITION",
    ] {
        assert!(system.contains(expected), "{expected}");
    }
    assert!(system.contains("Renderer"));
    assert!(system.contains("\"id\":\"provider\""));
    assert!(system.contains("catalog_deferred_after_selection"));
    assert_eq!(entries, before);
    assert_eq!(original_tools[0].description, "KEEP LIVE DESCRIPTION");
    assert_eq!(original_tools[0].input_schema["required"], json!(["exact"]));
    assert_eq!(serde_json::to_value(discovery).unwrap()["index"], index);
}
#[test]
fn reading_authoring_skill_keeps_other_authorized_service_identities_visible() {
    let mut value = index();
    value["items"].as_array_mut().unwrap().push(json!({"kind":"capability","id":"retrieval-service","name":"Web retrieval","revision":2,"tool_count":4,"description":"VERBOSE RETRIEVAL DESCRIPTION ".repeat(100)}));
    let discovery = InitialDiscovery::new(value, skills());
    let system = discovery.system(
        "policy",
        &[
            JournalEntry::User("current report".into()),
            root("SKILL.md", 3),
        ],
        &tools(),
    );
    assert!(system.contains("retrieval-service"));
    assert!(system.contains("Web retrieval"));
    assert!(!system.contains("VERBOSE RETRIEVAL DESCRIPTION"));
    assert!(!system.contains("Service recognition metadata."));
}
#[test]
fn no_selection_failed_activation_unloaded_names_and_new_turn_keep_full_metadata() {
    let discovery = InitialDiscovery::new(index(), skills());
    for entries in [
        vec![JournalEntry::User("work".into())],
        vec![JournalEntry::User("work".into()), activation(true)],
        vec![activation(false), JournalEntry::User("new task".into())],
        vec![exchange(
            "activate_tools",
            json!({}),
            json!({"loaded":["other"],"executed":false}),
        )],
    ] {
        assert_eq!(
            discovery.system("system", &entries, &tools()),
            full("system")
        );
    }
}
#[test]
fn live_tools_must_remain_loaded_including_after_checkpoint_or_finalization() {
    let discovery = InitialDiscovery::new(index(), skills());
    let entries = vec![JournalEntry::User("work".into()), activation(false)];
    assert_eq!(discovery.system("system", &entries, &[]), full("system"));
}
#[test]
fn failure_restores_full_index_for_alternative_discovery() {
    let discovery = InitialDiscovery::new(index(), skills());
    for error in [
        json!({"isError":true,"reason":"unavailable"}),
        json!({"is_error":true}),
    ] {
        let entries = vec![
            JournalEntry::User("work".into()),
            activation(false),
            exchange("render", json!({"exact":1}), error),
        ];
        assert_eq!(
            discovery.system("system", &entries, &tools()),
            full("system")
        );
    }
}
#[test]
fn only_matching_root_revision_retires_skill_recognition_with_auto_entry_tools() {
    let discovery = InitialDiscovery::new(index(), skills());
    for (path, revision, removed) in [
        ("SKILL.md", 3, true),
        ("SKILL.md", 4, false),
        ("references/guide.md", 3, false),
    ] {
        let entries = vec![
            JournalEntry::User("work".into()),
            root(path, revision),
            activation(false),
        ];
        let text = discovery.system("system", &entries, &tools());
        assert_eq!(!text.contains("SELECTED RECOGNITION"), removed);
        assert!(text.contains("KEEP UNREAD RECOGNITION"));
    }
    let entries = vec![JournalEntry::User("work".into()), root("SKILL.md", 3)];
    assert!(
        !discovery
            .system("system", &entries, &tools())
            .contains("SELECTED RECOGNITION")
    );
}
#[test]
fn malformed_catalog_and_foreign_tool_metadata_are_not_reinterpreted() {
    let malformed = json!({"items":[{"description":"KEEP MALFORMED"}],"note":"KEEP"});
    let discovery = InitialDiscovery::new(malformed, vec![]);
    assert!(
        discovery
            .system("system", &[activation(false)], &tools())
            .contains("KEEP MALFORMED")
    );
    let discovery = InitialDiscovery::new(index(), skills());
    let entries = vec![
        JournalEntry::User("work".into()),
        exchange(
            "remote_mcp",
            json!({}),
            json!({"loaded":["render"],"executed":false,"initial_discovery":{"retire":true}}),
        ),
    ];
    assert_eq!(
        discovery.system("system", &entries, &tools()),
        full("system")
    );
}
#[test]
fn very_small_catalog_keeps_original_when_retrieval_metadata_would_increase_it() {
    let mut value = index();
    value["items"][0]["description"] = json!("Short.");
    let discovery = InitialDiscovery::new(value.clone(), vec![]);
    let result = discovery.system("system", &[activation(false)], &tools());
    assert!(result.ends_with(&value.to_string()));
}
#[test]
fn restored_cursor_preserves_budgets_and_reuses_frozen_host_metadata() {
    let mut state = EngineState::new(
        vec![JournalEntry::User("original".into()), activation(false)],
        3,
    )
    .unwrap();
    let discovery = InitialDiscovery::new(index(), skills());
    state.model_calls = 2;
    state.recovery_tokens = 123;
    let original = serde_json::to_value(&state).unwrap();
    let restored: EngineState = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(restored.model_calls, 2);
    assert_eq!(restored.recovery_tokens, 123);
    assert_eq!(
        discovery.system("system", &restored.entries, &tools()),
        discovery.system("system", &state.entries, &tools())
    );
    assert!(original.get("initial_discovery").is_none());
}
#[test]
fn policy_text_and_authored_business_data_are_never_parsed_or_modified() {
    let discovery = InitialDiscovery::new(index(), skills());
    let policy = "User policy: Authorized capability index: NEVER REINTERPRET THIS TEXT";
    let entries = vec![
        JournalEntry::User("EXACT REQUIREMENTS".into()),
        activation(false),
        exchange(
            "render",
            json!({"description":"EXACT CODE AND JSON","array":[true,2,null]}),
            json!({"description":"EXACT BUSINESS RESULT"}),
        ),
    ];
    let before = entries.clone();
    assert!(
        discovery
            .system(policy, &entries, &tools())
            .starts_with(policy)
    );
    assert_eq!(entries, before);
}
