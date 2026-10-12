use super::{Binding, Catalog};
use crate::native;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
fn fixture() -> Catalog {
    let tool = native::sandbox_tool();
    let binding = Binding {
        capability: "tool:native:sandbox".into(),
        revision: 1,
        raw_name: tool.name.clone(),
        tool,
    };
    Catalog {
        profile: json!({}),
        records: BTreeMap::new(),
        bindings: [(binding.tool.name.clone(), binding)].into(),
        active: BTreeSet::new(),
        read: BTreeSet::new(),
        discovery_key: None,
        execution_selected: false,
        model_choices: BTreeMap::new(),
    }
}
#[test]
fn skill_execution_guidance_translates_legacy_names_without_granting_tools() {
    let mut catalog = fixture();
    let guidance = catalog.execution_guidance();
    let contract = &guidance["native_workspace_contract"];
    assert!(
        contract["runtime"]
            .as_str()
            .unwrap()
            .contains("runtime=javascript")
    );
    assert!(
        contract["execute"]
            .as_str()
            .unwrap()
            .contains("omit skill_id and files")
    );
    assert!(
        contract["skill_entrypoint"]
            .as_str()
            .unwrap()
            .contains("skill/<package-relative-path>")
    );
    assert!(
        contract["legacy_names"]
            .as_str()
            .unwrap()
            .contains("run_script maps to sandbox_run")
    );
    assert!(catalog.active.is_empty());
    catalog.bindings.clear();
    assert!(catalog.execution_guidance()["native_workspace_contract"].is_null());
}
#[test]
fn durable_workflow_selects_only_authorized_unique_dependencies_without_execution() {
    let mut catalog = fixture();
    let source = catalog.bindings["sandbox_run"].clone();
    let mut voice = source.clone();
    voice.capability = "tool:model:audio".into();
    voice.raw_name = "model.voice".into();
    voice.tool.name = "voice".into();
    catalog.bindings.insert("voice".into(), voice.clone());
    let directive = json!({"version":1,"status":"required","context_scope":"checkpoint",
        "required_tool":"sandbox_run","checkpoint":{"id":"immutable"},
        "prerequisite_tools":[{"capability_id":"historical-absent-id","tool":"model.voice"}]});
    let names = catalog.resolve_workflow(&source, &directive).unwrap();
    assert_eq!(names, vec!["sandbox_run", "voice"]);
    let schemas = catalog.workflow_schemas(&names).unwrap();
    assert_eq!(schemas.len(), crate::native::base().len() + 2);
    assert!(!catalog.requires_execution()); // Merely projecting never runs work.
    assert!(catalog.binding("voice").is_some());
    assert_eq!(
        catalog.resolve_workflow(
            &source,
            &json!({"version":1,"status":"complete","context_scope":"checkpoint"})
        ),
        Some(vec![])
    );
    assert_eq!(
        catalog.workflow_schemas(&[]).unwrap().len(),
        crate::native::base().len()
    );
    catalog.activate(&["voice".into()]).unwrap();
    assert_eq!(catalog.schemas().len(), crate::native::base().len() + 1);
    let mut duplicate = voice;
    duplicate.tool.name = "other_voice".into();
    duplicate.capability = "other-provider".into();
    catalog.bindings.insert("other_voice".into(), duplicate);
    assert!(catalog.resolve_workflow(&source, &directive).is_none());
    let crossed = json!({"version":1,"status":"required","context_scope":"checkpoint","required_tool":"voice","checkpoint":{}});
    assert!(catalog.resolve_workflow(&source, &crossed).is_none());
}
#[test]
fn multiword_queries_rank_partial_matches_and_empty_lists_tools_first() {
    let catalog = fixture();
    for query in [
        "sandbox execute code",
        "workspace run_script",
        "tool:native:sandbox",
    ] {
        assert_eq!(catalog.search(query, 0)["items"][0]["name"], "sandbox_run");
    }
    assert_eq!(catalog.search("", 0)["total"], 1);
    assert_eq!(catalog.search("unknown_unmatched", 0)["total"], 0);
}
#[test]
fn unique_provider_id_resolves_without_broadening_permission() {
    let mut catalog = fixture();
    catalog.activate(&["tool:native:sandbox".into()]).unwrap();
    assert!(catalog.binding("sandbox_run").is_some());
    assert!(catalog.activate(&["unregistered".into()]).is_err());
    assert!(catalog.binding("sandbox_run").is_some());
}

fn mv_selection_fixture() -> (Catalog, Vec<String>, Vec<String>) {
    let mut catalog = fixture();
    catalog.activate(&["sandbox_run".into()]).unwrap();
    let provider = "mcp_server:global:_:media-workflow";
    let raw = [
        "create_music_video_plan",
        "write_music_video_plan_overview",
        "write_music_video_plan_segment",
        "patch_music_video_plan_segment",
        "patch_music_video_plan_overview",
        "render_validated_music_video",
    ];
    let historical = [
        "mcp_create_music_video_plan_d962ea2fbc288d8c",
        "mcp_write_music_video_plan_overview_d28c7aa79b6831e4",
        "mcp_write_music_video_plan_segment_882a8609221391bc",
        "mcp_patch_music_video_plan_segment_ed4aa47ea675e11e",
        "mcp_patch_music_video_plan_overview_4d835ba775e7559a",
        "mcp_render_validated_music_video_71ac1a1433a3c04b",
    ];
    for (name, expected) in raw.iter().zip(historical) {
        let mut binding = catalog.bindings["sandbox_run"].clone();
        binding.capability = provider.into();
        binding.raw_name = (*name).into();
        binding.tool.name = crate::public_name(provider, name);
        assert_eq!(binding.tool.name, expected); // IDs before this change.
        catalog.bindings.insert(binding.tool.name.clone(), binding);
    }
    let complete: Vec<String> = historical.iter().map(|s| (*s).into()).collect();
    let mut incomplete = complete.clone();
    for i in 1..5 {
        incomplete[i] = format!("mcp_{}", raw[i]);
    }
    (catalog, complete, incomplete)
}

#[test]
fn production_mixed_incomplete_selection_corrects_entire_batch_without_loading_or_changing_ids() {
    let (mut catalog, complete, incomplete) = mv_selection_fixture();
    let before = catalog.active.clone();
    let error = catalog.activate_with_mode(&incomplete, "add").unwrap_err();
    let crate::ToolError::CorrectableInput(message) = error else {
        panic!("expected one bounded before-execution correction");
    };
    assert_eq!(catalog.active, before);
    assert!(message.contains("\"mode\":\"add\""));
    for name in &complete {
        assert!(message.contains(name), "{name}");
    }
    catalog.activate_with_mode(&complete, "add").unwrap();
    assert_eq!(catalog.active.len(), 7);
    assert!(catalog.active.contains("sandbox_run"));
    catalog.activate(&complete).unwrap();
    assert_eq!(catalog.active.len(), 6);
    assert!(!catalog.active.contains("sandbox_run"));
}

#[test]
fn incomplete_labels_never_select_among_providers_or_sanitization_collisions() {
    let mut catalog = fixture();
    catalog.activate(&["sandbox_run".into()]).unwrap();
    for (provider, raw) in [("one", "find-a"), ("two", "find_a"), ("three", "find-a")] {
        let mut binding = catalog.bindings["sandbox_run"].clone();
        binding.capability = provider.into();
        binding.raw_name = raw.into();
        binding.tool.name = crate::public_name(provider, raw);
        catalog.bindings.insert(binding.tool.name.clone(), binding);
    }
    let before = catalog.active.clone();
    let error = catalog.activate(&["mcp_find_a".into()]).unwrap_err();
    let crate::ToolError::CorrectableInput(message) = error else {
        panic!("ambiguous label must require explicit discovery");
    };
    assert!(message.contains("\"next_call\":null"));
    assert!(message.contains("\"names\":[\"find-a\"]"));
    assert!(message.contains("\"names\":[\"find_a\"]"));
    assert_eq!(catalog.active, before);
}

#[test]
fn unknown_typo_stale_id_and_mixed_unauthorized_names_require_discovery_without_loading() {
    let (mut catalog, complete, incomplete) = mv_selection_fixture();
    let before = catalog.active.clone();
    for invalid in [
        "mcp_write_music_video_plan_overvie",
        "mcp_write_music_video_plan_overview_0000000000000000",
        "mcp_private_operation",
    ] {
        let mut batch = incomplete.clone();
        batch.push(invalid.into());
        let crate::ToolError::CorrectableInput(message) = catalog.activate(&batch).unwrap_err()
        else {
            panic!("unknown identity must require discovery without loading");
        };
        assert!(message.contains(&format!("\"unresolved_names\":[\"{invalid}\"]")));
        assert!(message.contains(&format!("\"query\":\"{invalid}\"")));
        assert!(message.contains("\"next_call\":null"));
        for name in &complete {
            assert!(message.contains(name));
        }
        assert_eq!(catalog.active, before);
    }
    let raw: Vec<_> = complete
        .iter()
        .map(|name| catalog.bindings[name].raw_name.clone())
        .collect();
    catalog.activate(&raw).unwrap();
    assert_eq!(catalog.active, complete.into_iter().collect()); // Legacy raw names still work.
}
#[test]
fn ambiguous_provider_id_never_activates_every_tool_or_changes_existing_set() {
    let mut catalog = fixture();
    catalog.activate(&["sandbox_run".into()]).unwrap();
    let mut other = catalog.bindings["sandbox_run"].clone();
    other.tool.name = "another".into();
    catalog.bindings.insert("another".into(), other);
    assert!(catalog.activate(&["tool:native:sandbox".into()]).is_err());
    assert_eq!(catalog.active.len(), 1);
    assert!(!catalog.active.contains("another"));
}

#[test]
fn native_raw_names_resolve_only_when_unique_and_never_replace_on_ambiguity() {
    let mut catalog = fixture();
    let mut mcp = catalog.bindings["sandbox_run"].clone();
    mcp.capability = "mcp_server:global:_:search-provider".into();
    mcp.raw_name = "search".into();
    mcp.tool.name = "mcp_search_scoped".into();
    catalog.bindings.insert(mcp.tool.name.clone(), mcp.clone());
    catalog.activate(&["search".into()]).unwrap();
    assert!(catalog.binding("mcp_search_scoped").is_some());
    assert!(catalog.binding("search").is_none());
    mcp.tool.name = "another_scoped_search".into();
    mcp.capability = "mcp_server:global:_:other".into();
    catalog.bindings.insert(mcp.tool.name.clone(), mcp);
    assert!(catalog.activate(&["search".into()]).is_err());
    assert!(catalog.binding("mcp_search_scoped").is_some());
    assert!(catalog.binding("another_scoped_search").is_none());
}

#[test]
fn chinese_discovery_finds_native_execution_and_guides_lexical_misses() {
    let catalog = fixture();
    for query in ["绘制图表 数据可视化 生成图形", "网页生成 HTML 页面 宣传"] {
        assert_eq!(
            catalog.search_kind(query, 0, Some("tool"))["items"][0]["name"],
            "sandbox_run"
        );
    }
    let result = catalog.search_kind("unmatched", 0, Some("tool"));
    assert_eq!(result["total"], 0);
    assert_eq!(
        result["tool_discovery"]["arguments"],
        json!({"query":"","kind":"capability"})
    );
    assert_eq!(catalog.search_kind("", 0, Some("skill"))["total"], 0);
    assert_eq!(
        catalog.execution_guidance()["workspace_tools"],
        json!(["sandbox_run"])
    );
}

#[test]
fn unauthorized_workspace_is_not_in_execution_guidance_or_search() {
    let mut catalog = fixture();
    catalog.bindings.clear();
    assert_eq!(catalog.execution_guidance()["workspace_available"], false);
    assert_eq!(catalog.execution_guidance()["workspace_tools"], json!([]));
    assert_eq!(catalog.search_kind("", 0, Some("tool"))["total"], 0);
    assert!(catalog.activate(&["sandbox_run".into()]).is_err());
}

#[test]
fn batch_add_replace_and_failure_are_atomic_without_loading_unrequested_schemas() {
    let mut catalog = fixture();
    let mut other = catalog.bindings["sandbox_run"].clone();
    other.tool.name = "stage_check".into();
    other.raw_name = "check".into();
    catalog.bindings.insert(other.tool.name.clone(), other);
    catalog.activate(&["sandbox_run".into()]).unwrap();
    assert_eq!(
        catalog.search_kind("check", 0, Some("tool"))["items"][0]["schema_loaded"],
        false
    );
    catalog
        .activate_with_mode(&["check".into()], "add")
        .unwrap();
    assert_eq!(
        catalog.active,
        BTreeSet::from(["sandbox_run".into(), "stage_check".into()])
    );
    assert!(
        catalog
            .activate_with_mode(&["not_authorized".into()], "add")
            .is_err()
    );
    assert!(
        catalog
            .activate_with_mode(&["stage_check".into()], "unknown")
            .is_err()
    );
    assert_eq!(catalog.active.len(), 2);
    catalog.activate(&["stage_check".into()]).unwrap(); // Historical default.
    assert_eq!(catalog.active, BTreeSet::from(["stage_check".into()]));
    assert!(catalog.binding("sandbox_run").is_none());
    catalog
        .activate_with_mode(&["sandbox_run".into(), "stage_check".into()], "replace")
        .unwrap();
    assert_eq!(catalog.schemas().len(), crate::native::base().len() + 2);
    catalog.activate(&[]).unwrap();
    assert!(catalog.active.is_empty());
}

#[test]
fn add_overflow_rejects_without_mutating_previously_loaded_set() {
    let mut catalog = fixture();
    for i in 0..32 {
        let mut binding = catalog.bindings["sandbox_run"].clone();
        binding.tool.name = format!("tool_{i}");
        catalog.bindings.insert(binding.tool.name.clone(), binding);
    }
    let names: Vec<_> = (0..32).map(|i| format!("tool_{i}")).collect();
    catalog.activate(&names).unwrap();
    let before = catalog.active.clone();
    assert!(
        catalog
            .activate_with_mode(&["sandbox_run".into()], "add")
            .is_err()
    );
    assert_eq!(catalog.active, before);
}

#[test]
fn exact_raw_name_ranks_before_unrelated_word_matches_and_preserves_pagination() {
    let mut catalog = fixture();
    let mut exact = catalog.bindings["sandbox_run"].clone();
    exact.tool.name = "zz_authorized_render".into();
    exact.raw_name = "render_named_output".into();
    catalog
        .bindings
        .insert(exact.tool.name.clone(), exact.clone());
    for i in 0..20 {
        let mut other = exact.clone();
        other.tool.name = format!("aa_word_match_{i:02}");
        other.raw_name = "other_operation".into();
        other.tool.description = "render named output ".repeat(1000);
        catalog.bindings.insert(other.tool.name.clone(), other);
    }
    let page = catalog.search_kind("render named output", 0, Some("tool"));
    assert_eq!(page["items"][0]["name"], exact.tool.name);
    assert_eq!(page["total"], 22);
    assert_eq!(page["next_offset"], 12);
    let next = catalog.search_kind("render named output", 12, Some("tool"));
    assert_eq!(next["items"].as_array().unwrap().len(), 10);
    assert_eq!(next["next_offset"], serde_json::Value::Null);
    assert!(catalog.active.is_empty());
}

fn many_tools() -> Catalog {
    let mut catalog = fixture();
    catalog.bindings.clear();
    for i in 0..68 {
        let group = if i < 8 {
            "mcp:workflow"
        } else {
            "mcp:unrelated"
        };
        let raw = if i == 0 {
            "create_plan".into()
        } else if i == 1 {
            "render_plan".into()
        } else {
            format!("operation_{i}")
        };
        let name = format!("public_{raw}");
        catalog.bindings.insert(name.clone(), Binding {
            capability: group.into(), revision: 3, raw_name: raw,
            tool: exoharness_contracts::model::Tool { name, description: format!("{group} operation {i}"), input_schema: json!({"type":"object","properties":{"payload":{"description":"FULL SCHEMA MUST NOT LEAK"}}}) },
        });
    }
    catalog
}

#[test]
fn sixty_eight_tools_are_selected_by_service_then_loaded_without_catalog_traversal() {
    let mut catalog = many_tools();
    let index = catalog
        .discover(&json!({"query":"","kind":"tool"}))
        .unwrap();
    assert_eq!(index["kind"], "capability");
    assert_eq!(index["total"], 2);
    assert_eq!(index["items"].as_array().unwrap().len(), 2);
    assert!(!index.to_string().contains("input_schema"));
    assert!(!index.to_string().contains("operation_67"));
    let scoped = catalog
        .discover(&json!({"query":"","kind":"tool","capability_id":"mcp:workflow"}))
        .unwrap();
    assert_eq!(scoped["total"], 8);
    assert!(scoped["next_page"].is_null());
    assert!(
        scoped["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["capability_id"] == "mcp:workflow")
    );
    assert!(catalog.active.is_empty());
    assert!(!catalog.requires_execution());
    assert_eq!(catalog.schemas().len(), native::base().len());
    catalog.activate(&["create_plan".into()]).unwrap();
    let schemas = catalog.schemas();
    assert_eq!(schemas.len(), native::base().len() + 1);
    assert!(schemas.iter().any(|s| {
        s.name == "public_create_plan"
            && s.input_schema
                .to_string()
                .contains("FULL SCHEMA MUST NOT LEAK")
    }));
    assert!(!schemas.iter().any(|s| s.name == "public_render_plan"));
    assert!(
        catalog
            .discover(&json!({"query":"","kind":"tool","capability_id":"mcp:forbidden"}))
            .is_err()
    );
    assert_eq!(catalog.active.len(), 1);
}

#[test]
fn changed_query_or_scope_resets_offset_and_cursor_without_skipping_exact_matches() {
    let mut catalog = many_tools();
    let initial = catalog
        .discover(&json!({"query":"operation","kind":"tool"}))
        .unwrap();
    assert_eq!(initial["next_offset"], 12);
    let next = catalog.discover(&initial["next_page"]).unwrap();
    assert_eq!(next["offset"], 12);
    assert_eq!(next["offset_reset"], false);
    let changed = catalog
        .discover(&json!({"query":"render_plan","kind":"tool","offset":24}))
        .unwrap();
    assert_eq!(changed["offset"], 0);
    assert_eq!(changed["offset_reset"], true);
    assert_eq!(changed["items"][0]["name"], "public_render_plan");
    let mut wrong_cursor = initial["next_page"].clone();
    wrong_cursor["query"] = json!("create_plan");
    let reset = catalog.discover(&wrong_cursor).unwrap();
    assert_eq!(reset["offset_reset"], true);
    assert_eq!(reset["items"][0]["name"], "public_create_plan");
    let reset = catalog
        .discover(&json!({"query":"","kind":"tool","capability_id":"mcp:workflow","offset":60}))
        .unwrap();
    assert_eq!(reset["offset"], 0);
    assert_eq!(reset["total"], 8);
    let first = catalog
        .discover(&json!({"query":"operation","kind":"tool"}))
        .unwrap();
    catalog
        .bindings
        .get_mut("public_create_plan")
        .unwrap()
        .revision += 1;
    assert_eq!(
        catalog.discover(&first["next_page"]).unwrap()["offset_reset"],
        true
    );
}

#[test]
fn exact_batch_names_are_complete_and_ambiguity_is_not_an_implicit_grant() {
    let mut catalog = many_tools();
    let mut duplicate = catalog.bindings["public_render_plan"].clone();
    duplicate.capability = "mcp:other".into();
    duplicate.tool.name = "other_render".into();
    catalog
        .bindings
        .insert(duplicate.tool.name.clone(), duplicate);
    let batch = catalog.discover(&json!({"query":"irrelevant lexical words","kind":"tool","names":["render_plan","create_plan","missing"]})).unwrap();
    assert_eq!(batch["items"].as_array().unwrap().len(), 3);
    assert_eq!(batch["missing_names"], json!(["missing"]));
    assert!(!batch.to_string().contains("FULL SCHEMA MUST NOT LEAK"));
    assert!(catalog.activate(&["render_plan".into()]).is_err());
    assert!(catalog.active.is_empty());
    let scoped = catalog.discover(&json!({"query":"","kind":"tool","capability_id":"mcp:workflow","names":["render_plan","create_plan"]})).unwrap();
    assert_eq!(scoped["total"], 2);
    let names: Vec<String> = scoped["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_owned())
        .collect();
    catalog.activate(&names).unwrap();
    assert_eq!(catalog.active.len(), 2);
}

#[test]
fn initial_service_index_is_bounded_and_its_continuation_does_not_restart() {
    let mut catalog = many_tools();
    for (index, binding) in catalog.bindings.values_mut().enumerate() {
        binding.capability = format!("mcp:provider_{}", index / 5);
    }
    let first = catalog.capability_index();
    assert_eq!(first["total"], 14);
    assert_eq!(first["items"].as_array().unwrap().len(), 12);
    let next = catalog.discover(&first["next_page"]).unwrap();
    assert_eq!(next["items"].as_array().unwrap().len(), 2);
    assert_eq!(next["offset_reset"], false);
    assert!(next["next_page"].is_null());
    let legacy = catalog
        .discover(&json!({"query":"","kind":"capability","offset":12}))
        .unwrap();
    assert_eq!(legacy["items"], next["items"]);
    assert!(catalog.active.is_empty());
}

#[test]
fn legacy_mixed_query_next_page_is_valid_native_input_and_preserves_the_query() {
    let mut catalog = many_tools();
    let first = catalog.discover(&json!({"query":"operation"})).unwrap();
    let schema = native::base()
        .into_iter()
        .find(|t| t.name == "search_capabilities")
        .unwrap()
        .input_schema;
    assert!(crate::validate(&schema, &first["next_page"]).is_ok());
    assert!(first["next_page"].get("kind").is_none());
    let second = catalog.discover(&first["next_page"]).unwrap();
    assert_eq!(second["offset"], 12);
    assert_eq!(second["offset_reset"], false);
}

#[test]
fn unauthorized_native_name_is_not_a_correctable_alias_and_keeps_loaded_set() {
    let mut catalog = fixture();
    catalog.activate(&["sandbox_run".into()]).unwrap();
    let loaded = catalog.active.clone();
    assert!(matches!(
        catalog.activate(&["sandbox_run".into(), "coding_apply_patch".into()]),
        Err(crate::ToolError::Authorization)
    ));
    assert_eq!(catalog.active, loaded);
}
