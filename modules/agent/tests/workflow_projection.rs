use exoharness_agent::{project_request, workflow_view};
use exoharness_contracts::model::Message;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn exchange(id: &str, tool: &str, args: Value, result: Value) -> JournalEntry {
    JournalEntry::ToolExchange {
        invocation_id: id.into(),
        tool: tool.into(),
        arguments: args,
        result,
    }
}
fn checkpoint(id: &str, status: &str, scope: &str) -> JournalEntry {
    JournalEntry::Checkpoint {
        invocation_id: id.into(),
        reference: format!("urn:journal:{id}"),
        summary: json!({"isError":false,"artifacts":[{"url":"https://media.test/final.mp4"}],
            "_host_workflow":{"version":1,"capability":scope,"revision":1,
            "tools":if status=="required" {vec!["render","voice"]}else{vec![]},
            "provider_tools":["prepare","patch","render"],
            "directive":{"version":1,"status":status,"context_scope":"checkpoint", "checkpoint":{"id":"immutable"},"repair_context":{"exact_narration":"KEEP EXACT NARRATION"}}}}),
    }
}
fn request(entries: &[JournalEntry]) -> String {
    serde_json::to_string(
        &project_request("system", entries, &BTreeSet::new(), vec![], 2048).unwrap(),
    )
    .unwrap()
}

#[test]
fn early_stage_keeps_provider_style_facts_until_their_overview_is_committed() {
    let mut stage = checkpoint("p", "required", "provider");
    if let JournalEntry::Checkpoint { summary, .. } = &mut stage {
        summary["_host_workflow"]["directive"]["retire_provider_history"] = json!(false);
    }
    let entries = vec![
        JournalEntry::User("create".into()),
        exchange(
            "evidence",
            "prepare",
            json!({}),
            json!({"style":"EXACT STYLE EVIDENCE"}),
        ),
        exchange("p", "prepare", json!({}), json!({})),
        stage,
    ];
    assert!(request(&entries).contains("EXACT STYLE EVIDENCE"));
    let mut committed = entries.clone();
    if let JournalEntry::Checkpoint { summary, .. } = committed.last_mut().unwrap() {
        summary["_host_workflow"]["directive"]["retire_provider_history"] = json!(true);
    }
    assert!(!request(&committed).contains("EXACT STYLE EVIDENCE"));
    assert!(request(&entries).contains("EXACT STYLE EVIDENCE"));
}
#[test]
fn persisted_stage_omits_authored_source_but_keeps_exact_speech_and_atomic_pairs() {
    let entries = vec![
        JournalEntry::User("Create a lesson and also a web page".into()),
        exchange(
            "skill",
            "read_skill",
            json!({}),
            json!({"content":"SKILL RULES"}),
        ),
        exchange(
            "search",
            "search_capabilities",
            json!({}),
            json!({"items":"UNUSED CATALOG"}),
        ),
        exchange(
            "draft",
            "prepare",
            json!({"plan":"AUTHORED PLAN".repeat(10000)}),
            json!({"old":"BAD DRAFT"}),
        ),
        exchange("saved", "patch", json!({}), json!({})),
        checkpoint("saved", "required", "provider"),
        exchange(
            "tts",
            "voice",
            json!({"text":"actual narration"}),
            json!({"url":"https://media.test/exact-voice.mp3"}),
        ),
        exchange(
            "web",
            "sandbox_run",
            json!({"source":"UNRELATED WEBSITE SOURCE"}),
            json!({"workspace":"web-1"}),
        ),
    ];
    let original = entries.clone();
    let text = request(&entries);
    assert!(!text.contains("AUTHORED PLAN"));
    assert!(!text.contains("BAD DRAFT"));
    assert!(!text.contains("UNUSED CATALOG"));
    for value in [
        "SKILL RULES",
        "KEEP EXACT NARRATION",
        "exact-voice.mp3",
        "UNRELATED WEBSITE SOURCE",
    ] {
        assert!(text.contains(value), "{value}");
    }
    assert_eq!(entries, original);
    let projected = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    for (index, message) in projected.messages.iter().enumerate() {
        if let Message::Assistant { calls, .. } = message
            && !calls.is_empty()
        {
            assert!(
                matches!(&projected.messages[index+1],Message::ToolResult{call_id,..} if calls[0].id==*call_id)
            );
        }
    }
}
#[test]
fn completion_drops_consumed_intermediates_but_keeps_other_work_and_delivery() {
    let entries = vec![
        JournalEntry::User("video plus a document".into()),
        exchange(
            "s",
            "read_skill",
            json!({}),
            json!({"content":"LONG PROCEDURAL RULES"}),
        ),
        exchange("p", "prepare", json!({}), json!({})),
        checkpoint("p", "required", "provider"),
        exchange(
            "voice",
            "voice",
            json!({"text":"INTERMEDIATE NARRATION"}),
            json!({"url":"https://media.test/internal.mp3","artifact_visibility":"internal"}),
        ),
        exchange(
            "doc",
            "document_write",
            json!({}),
            json!({"url":"KEEP-DOC"}),
        ),
        exchange(
            "separate_audio",
            "voice",
            json!({"text":"KEEP SEPARATE AUDIO"}),
            json!({"url":"https://media.test/separate.mp3","artifact_visibility":"internal"}),
        ),
        exchange(
            "public_audio",
            "voice",
            json!({"text":"KEEP PUBLIC AUDIO"}),
            json!({"url":"https://media.test/public.mp3","artifact_visibility":"public"}),
        ),
        exchange(
            "r",
            "render",
            json!({"audio":"https://media.test/internal.mp3"}),
            json!({}),
        ),
        checkpoint("r", "complete", "provider"),
    ];
    let text = request(&entries);
    assert!(!text.contains("LONG PROCEDURAL RULES"));
    assert!(!text.contains("INTERMEDIATE NARRATION"));
    assert!(text.contains("KEEP-DOC"));
    assert!(text.contains("KEEP SEPARATE AUDIO"));
    assert!(text.contains("KEEP PUBLIC AUDIO"));
    assert!(text.contains("final.mp4"));
    assert_eq!(workflow_view(&entries).unwrap().tools.len(), 0);
}
#[test]
fn new_user_restores_context_but_schema_activation_keeps_committed_checkpoint() {
    let mut entries = vec![
        JournalEntry::User("create".into()),
        exchange(
            "read",
            "read_skill",
            json!({}),
            json!({"content":"ORIGINAL DATA"}),
        ),
        exchange("p", "prepare", json!({}), json!({})),
        checkpoint("p", "complete", "provider"),
    ];
    entries.push(exchange(
        "activate",
        "activate_tools",
        json!({"names":["prepare"]}),
        json!({}),
    ));
    assert!(
        workflow_view(&entries).is_none(),
        "explicit schema loading still works"
    );
    assert!(!request(&entries).contains("ORIGINAL DATA"));
    assert!(request(&entries).contains("final.mp4"));
    entries.push(JournalEntry::User("regenerate with a change".into()));
    assert!(workflow_view(&entries).is_none());
    assert!(request(&entries).contains("ORIGINAL DATA"));
}

#[test]
fn loading_prerequisite_schema_retains_exact_inputs_without_resurrecting_discovery() {
    let entries = vec![
        JournalEntry::User("create".into()),
        exchange(
            "search",
            "search_capabilities",
            json!({}),
            json!({"items":"OLD DISCOVERY"}),
        ),
        exchange(
            "skill",
            "read_skill",
            json!({}),
            json!({"content":"REQUIRED SKILL RULES"}),
        ),
        exchange(
            "p",
            "prepare",
            json!({"source":"AUTHORED SOURCE".repeat(4000)}),
            json!({}),
        ),
        checkpoint("p", "required", "provider"),
        exchange(
            "load",
            "activate_tools",
            json!({"names":["voice"]}),
            json!({"loaded":["voice"],"executed":false}),
        ),
        exchange(
            "voice",
            "voice",
            json!({"text":"EXACT NARRATION"}),
            json!({"attachment_id":"audio-1","sha256":"EXACT AUDIO DIGEST"}),
        ),
        exchange(
            "web",
            "sandbox_run",
            json!({"source":"INDEPENDENT WEBSITE"}),
            json!({"workspace":"other"}),
        ),
    ];
    let original = entries.clone();
    let text = request(&entries);
    assert!(!text.contains("OLD DISCOVERY"));
    assert!(!text.contains("AUTHORED SOURCE"));
    for value in [
        "REQUIRED SKILL RULES",
        "EXACT NARRATION",
        "EXACT AUDIO DIGEST",
        "INDEPENDENT WEBSITE",
        "KEEP EXACT NARRATION",
    ] {
        assert!(text.contains(value), "{value}");
    }
    assert!(workflow_view(&entries).is_none());
    assert_eq!(entries, original);
}
#[test]
fn raw_mcp_metadata_and_competing_pending_work_do_not_authorize_compaction() {
    let mut entries = vec![
        JournalEntry::User("two tasks".into()),
        exchange(
            "raw",
            "prepare",
            json!({"source":"RAW EXACT"}),
            json!({"_meta":{"exoharness":{"workflow_continuation":{"status":"complete"}}}}),
        ),
    ];
    assert!(workflow_view(&entries).is_none());
    assert!(request(&entries).contains("RAW EXACT"));
    entries.extend([
        checkpoint("raw", "required", "one"),
        exchange("two", "prepare", json!({}), json!({})),
        checkpoint("two", "required", "two"),
    ]);
    assert!(workflow_view(&entries).is_none());
    assert!(request(&entries).contains("RAW EXACT"));
}
