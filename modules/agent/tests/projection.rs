use exoharness_agent::project_request;
use exoharness_contracts::model::Message;
use exoharness_core::context::JournalEntry;
use serde_json::json;
use std::collections::BTreeSet;
#[test]
fn calls_and_results_stay_paired_and_exact_and_declared_checkpoint_replaces_both() {
    let exchange = JournalEntry::ToolExchange {
        invocation_id: "inv-1".into(),
        tool: "generic-tool".into(),
        arguments: json!({"source":"source-data-".repeat(1000)}),
        result: json!({"isError":false,"data":"x".repeat(10000)}),
    };
    let entries = vec![exchange.clone()];
    let full = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    assert_eq!(full.messages.len(), 3);
    assert!(
        matches!(&full.messages[1],Message::Assistant{calls,..} if calls[0].arguments["source"]=="source-data-".repeat(1000))
    );
    assert!(matches!(&full.messages[2],Message::ToolResult{call_id,..} if call_id=="inv-1"));
    let compact = project_request(
        "system",
        &[
            exchange.clone(),
            JournalEntry::Checkpoint {
                invocation_id: "inv-1".into(),
                reference: "store://exact/1".into(),
                summary: json!({"next":"continue"}),
            },
        ],
        &BTreeSet::new(),
        vec![],
        2048,
    )
    .unwrap();
    assert_eq!(compact.messages.len(), 2);
    assert!(
        matches!(&compact.messages[1],Message::RuntimeData{text} if text.contains("store://exact/1") && text.contains("continue") && !text.contains("source-data-") && !text.contains(&"x".repeat(1000)))
    );
    assert!(
        !serde_json::to_string(&compact)
            .unwrap()
            .contains("_persisted_arguments_reference")
    );
    assert_eq!(entries[0], exchange);
}

#[test]
fn checkpoint_keeps_workspace_handle_as_runtime_data_not_a_new_user_message() {
    let entries = vec![
        JournalEntry::User("Create deliverable".into()),
        JournalEntry::ToolExchange {
            invocation_id: "create-1".into(),
            tool: "workspace_create".into(),
            arguments: json!({}),
            result: json!({"workspace_id":"saved-id","revision":1}),
        },
        JournalEntry::Checkpoint {
            invocation_id: "create-1".into(),
            reference: "urn:inv:create-1".into(),
            summary: json!({"workspace_id":"saved-id","revision":1}),
        },
    ];
    let projected = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    assert_eq!(
        projected
            .messages
            .iter()
            .filter(|m| matches!(m, Message::User { .. }))
            .count(),
        1
    );
    assert!(
        matches!(projected.messages.last(),Some(Message::ToolResult{call_id,content,..}) if call_id=="create-1" && content.contains("saved-id") && content.contains("revision"))
    );
    assert!(matches!(&projected.messages[1],Message::User{text} if text=="Create deliverable"));
}
#[test]
fn selected_skill_revision_is_included_once_and_unselected_skills_are_absent() {
    let skill = JournalEntry::SkillLoaded {
        revision_key: "skill-a:2".into(),
        instructions: "Exact instructions".into(),
    };
    let entries = vec![
        skill.clone(),
        skill,
        JournalEntry::SkillLoaded {
            revision_key: "unused:1".into(),
            instructions: "Unused".into(),
        },
    ];
    let request = project_request(
        "system",
        &entries,
        &BTreeSet::from(["skill-a:2".into()]),
        vec![],
        2048,
    )
    .unwrap();
    assert_eq!(request.messages.len(), 2);
}

#[test]
fn distinct_durable_read_pages_and_external_data_stay_exact() {
    let mut entries = vec![JournalEntry::User("Continue the original task".into())];
    entries.push(JournalEntry::ToolExchange {
        invocation_id: "write-1".into(),
        tool: "generic_write".into(),
        arguments: json!({"files":{"file.txt":"model-authored source"}}),
        result: json!({"workspace_id":"saved","revision":3}),
    });
    entries.push(JournalEntry::Checkpoint {
        invocation_id: "write-1".into(),
        reference: "store://write-1".into(),
        summary: json!({"workspace_id":"saved","revision":3}),
    });
    for n in 0..5 {
        let result = json!({"workspace_id":"saved","revision":3,"file":{"path":"file.txt","sha256":"digest"},"offset":n*10,"next_offset":(n+1)*10,"content":format!("exact-body-{n}"),"_durable_read_body":true});
        entries.push(JournalEntry::ToolExchange {
            invocation_id: format!("read-{n}"),
            tool: "generic_read".into(),
            arguments: json!({"offset":n*10}),
            result: result.clone(),
        });
        entries.push(JournalEntry::Checkpoint {
            invocation_id: format!("read-{n}"),
            reference: format!("store://read-{n}"),
            summary: result,
        });
    }
    // Ordinary results, including external data, never opt into compaction.
    entries.push(JournalEntry::ToolExchange {
        invocation_id: "external".into(),
        tool: "other".into(),
        arguments: json!({}),
        result: json!({"content":"external exact data"}),
    });
    let original = entries.clone();
    let request = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    let serialized = serde_json::to_string(&request).unwrap();
    for n in 0..5 {
        assert!(serialized.contains(&format!("exact-body-{n}")));
        assert!(serialized.contains(&format!("store://read-{n}")));
    }
    assert!(serialized.contains("exact-body-3"));
    assert!(serialized.contains("exact-body-4"));
    assert!(serialized.contains("external exact data"));
    assert_eq!(entries, original);
    let inherited =
        project_request("system", &entries[3..], &BTreeSet::new(), vec![], 2048).unwrap();
    let inherited = serde_json::to_string(&inherited).unwrap();
    for n in 0..5 {
        assert!(inherited.contains(&format!("exact-body-{n}")));
    }
    assert_eq!(
        request
            .messages
            .iter()
            .filter(|m| matches!(m, Message::User { .. }))
            .count(),
        1
    );
    for (index, message) in request.messages.iter().enumerate() {
        if let Message::Assistant { calls, .. } = message
            && !calls.is_empty()
        {
            assert!(
                matches!(&request.messages[index+1],Message::ToolResult{call_id,..} if calls[0].id==*call_id)
            );
        }
    }
}

#[test]
fn mixed_size_checkpoints_preserve_create_then_edit_order_without_stale_tail() {
    let mut entries = vec![JournalEntry::User("Create and publish".into())];
    for (id, tool, args, revision) in [
        ("create", "workspace_create", json!({}), 1),
        (
            "write",
            "workspace_edit",
            json!({"files":{"index.html":"large source".repeat(1000)}}),
            2,
        ),
    ] {
        let summary = json!({"workspace_id":"same","revision":revision});
        entries.push(JournalEntry::ToolExchange {
            invocation_id: id.into(),
            tool: tool.into(),
            arguments: args,
            result: summary.clone(),
        });
        entries.push(JournalEntry::Checkpoint {
            invocation_id: id.into(),
            reference: format!("urn:{id}"),
            summary,
        });
    }
    let original = entries.clone();
    let request = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    let states = request
        .messages
        .iter()
        .filter_map(|m| match m {
            Message::RuntimeData { text } => serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .map(|v| v["completed_tool_checkpoint"]["result"].clone()),
            Message::ToolResult { content, .. } => {
                serde_json::from_str::<serde_json::Value>(content)
                    .ok()
                    .map(|v| v["result"].clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states[0]["revision"], 1);
    assert_eq!(states[1]["revision"], 2);
    assert!(matches!(
        request.messages.last(),
        Some(Message::RuntimeData { .. })
    ));
    assert!(
        !serde_json::to_string(&request)
            .unwrap()
            .contains("large source")
    );
    assert_eq!(entries, original);
}

#[test]
fn inline_mcp_media_is_referenced_without_changing_original_receipt_or_file_links() {
    let raw = json!({"isError":false,"content":[
        {"type":"text","text":"keep exact business text"},
        {"type":"resource_link","uri":"https://example.test/image.png","name":"image.png"},
        {"type":"image","mimeType":"image/png","data":"A".repeat(3_311_980)},
        {"type":"audio","mimeType":"audio/wav","data":"audio-bytes"}
    ],"structuredContent":{"id":"job-id","status":"succeeded","result":{"download_url":"https://example.test/image.png","sha256":"digest"}},"business_data":{"data":"exact authored data"}});
    let entries = vec![JournalEntry::ToolExchange {
        invocation_id: "media-receipt".into(),
        tool: "media-tool".into(),
        arguments: json!({"inline":true}),
        result: raw.clone(),
    }];
    let request = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    assert!(serde_json::to_vec(&request).unwrap().len() < 10_000);
    let Message::ToolResult {
        content,
        call_id,
        is_error,
    } = &request.messages[2]
    else {
        panic!("paired receipt missing")
    };
    let projected: serde_json::Value = serde_json::from_str(content).unwrap();
    assert_eq!(call_id, "media-receipt");
    assert!(!is_error);
    assert_eq!(projected["content"][0], raw["content"][0]);
    assert_eq!(projected["content"][1], raw["content"][1]);
    assert_eq!(projected["structuredContent"], raw["structuredContent"]);
    assert_eq!(projected["business_data"], raw["business_data"]);
    assert!(projected["content"][2].get("data").is_none());
    assert_eq!(
        projected["content"][2]["inline_media_reference"]["invocation_id"],
        "media-receipt"
    );
    assert_eq!(
        projected["content"][2]["inline_media_reference"]["encoded_bytes"],
        3_311_980
    );
    assert!(projected["content"][3].get("data").is_none());
    let JournalEntry::ToolExchange { result, .. } = &entries[0] else {
        unreachable!()
    };
    assert_eq!(result, &raw);
}

#[test]
fn embedded_binary_resource_is_referenced_but_text_resource_is_kept_exact() {
    let raw = json!({"content":[
        {"type":"resource","resource":{"uri":"saved://binary","mimeType":"image/png","blob":"B".repeat(2_000_000)}},
        {"type":"resource","resource":{"uri":"saved://code","mimeType":"text/plain","text":"exact original code"}}
    ],"isError":false});
    let entries = vec![JournalEntry::ToolExchange {
        invocation_id: "embedded-1".into(),
        tool: "read-resource".into(),
        arguments: json!({}),
        result: raw.clone(),
    }];
    let request = project_request("system", &entries, &BTreeSet::new(), vec![], 2048).unwrap();
    assert!(serde_json::to_vec(&request).unwrap().len() < 10000);
    let Message::ToolResult { content, .. } = &request.messages[2] else {
        panic!("missing receipt")
    };
    let projected: serde_json::Value = serde_json::from_str(content).unwrap();
    assert_eq!(projected["content"][1], raw["content"][1]);
    assert_eq!(projected["content"][0]["resource"]["uri"], "saved://binary");
    assert!(projected["content"][0]["resource"].get("blob").is_none());
    assert_eq!(
        projected["content"][0]["resource"]["inline_media_reference"]["encoded_bytes"],
        2_000_000
    );
}
