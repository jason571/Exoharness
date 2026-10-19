use super::*;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn ordinary_root_read_accepts_task_hint_and_preserves_canonical_cache() {
    let f = fixture(
        vec![
            // Exact shape of the production PPT failure: query without path.
            tool(
                "read_skill",
                json!({"id":SKILL,"query":"创建一份当前中国经济现状的分析报告PPT"}),
            ),
            tool(
                "read_skill",
                json!({"id":SKILL,"path":"SKILL.md","query":"different task hint"}),
            ),
            tool("read_skill", json!({"id":SKILL})),
        tool("read_skill", json!({"id":SKILL,"path":"scripts/create_document.cjs","query":"document rendering"})),
        tool("read_skill", json!({"id":SKILL,"path":"scripts/create_document.cjs"})),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let first = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert!(
        first["content"]
            .as_str()
            .unwrap()
            .contains("Use sandbox_run")
    );
    assert!(first.get("selected_resources").is_none());
    assert_eq!(first["_host_execution"], false);
    for sequence in [2, 3] {
        let repeated = invocation(&f, &session, &run, sequence)
            .await
            .result
            .unwrap();
        assert_eq!(repeated["already_loaded"], true);
        assert!(repeated.get("content").is_none());
    }
    let reference = invocation(&f, &session, &run, 4).await.result.unwrap();
    assert!(reference["content"].as_str().unwrap().contains("const fs="));
    assert_eq!(
        invocation(&f, &session, &run, 5).await.result.unwrap()["already_loaded"],
        true
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn query_hint_does_not_block_combined_loading_and_real_execution() {
    let f = fixture(vec![
        tool("read_skill", json!({"id":SKILL,"query":"PPT report","load_tools":["sandbox_run"]})),
        tool("sandbox_run", json!({"runtime":"python","entrypoint":"main.py",
            "files":{"main.py":"from pathlib import Path\nPath('result.txt').write_text('complete')\n"},"output_files":["result.txt"]})),
        answer(),
    ], true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let read = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(read["tool_selection"]["loaded"], json!(["sandbox_run"]));
    let result = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(result["execution"]["exit_code"], 0);
    assert_eq!(result["artifacts"].as_array().unwrap().len(), 1);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn missing_optional_index_is_correctable_then_primary_skill_executes() {
    let f = fixture(vec![
        tool("read_skill",json!({"id":SKILL,"path":"resource-index.json","query":"PPT"})),
        tool("read_skill",json!({"id":SKILL,"query":"PPT","load_tools":["sandbox_run"]})),
        tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py",
            "files":{"main.py":"from pathlib import Path\nPath('result.txt').write_text('complete')\n"},"output_files":["result.txt"]})),
        answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(rejected["_host_input_rejection"], 1);
    assert!(rejected.get("_host_authorization_denied").is_none());
    let read = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert!(read["content"].is_string());
    let result = invocation(&f, &session, &run, 3).await.result.unwrap();
    assert_eq!(result["execution"]["exit_code"], 0);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

async fn install(f: &Fixture) {
    let tools = f.tools.clone();
    tokio::task::spawn_blocking(move || {
        let id = "references/video.md";
        let text = "Use a verified final artifact receipt before delivering a video.";
        let index = json!({"version":1,"resources":[{"id":"video","path":id,"terms":["视频","video"],
            "sha256":format!("{:x}", Sha256::digest(text.as_bytes()))}]});
        let old = tools.registry.0.capability(&RegistryAccess::Administrator, SKILL).unwrap();
        let imported = tools.skills.import(&RegistryAccess::Administrator, &serde_json::from_value(json!({
            "expected_revision":old.revision,"files":[
                {"path":"SKILL.md","content_base64":STANDARD.encode("---\nname: document-fixture\ndescription: Indexed experience fixture.\n---\nRead resource-index.json with the current task query.\n")},
                {"path":"resource-index.json","content_base64":STANDARD.encode(index.to_string())},
                {"path":id,"content_base64":STANDARD.encode(text)}
            ]})).unwrap(), "admin").unwrap();
        tools.registry.0.set_capability_enabled(&RegistryAccess::Administrator, SKILL, imported.revision, true, "admin").unwrap();
    }).await.unwrap();
}

#[tokio::test]
async fn indexed_reads_load_only_matching_resources_and_preserve_root_and_query_cache() {
    let f = fixture(
        vec![
            tool("read_skill", json!({"id":SKILL,"query":"视频"})),
            tool(
                "read_skill",
                json!({"id":SKILL,"path":"resource-index.json","query":"生成视频"}),
            ),
            tool(
                "read_skill",
                json!({"id":SKILL,"path":"resource-index.json","query":"生成视频"}),
            ),
            tool(
                "read_skill",
                json!({"id":SKILL,"path":"resource-index.json","query":"music"}),
            ),
            answer(),
        ],
        true,
    )
    .await;
    install(&f).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let root = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert!(
        !root["content"]
            .as_str()
            .unwrap()
            .contains("verified final artifact")
    );
    let read = invocation(&f, &session, &run, 2).await.result.unwrap();
    let content: Value = serde_json::from_str(read["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["resources"][0]["path"], "references/video.md");
    assert!(read["selected_resources"][0].get("content").is_none());
    assert_eq!(read["_host_execution"], false);
    assert_eq!(
        invocation(&f, &session, &run, 3).await.result.unwrap()["already_loaded"],
        true
    );
    let other = invocation(&f, &session, &run, 4).await.result.unwrap();
    assert_ne!(other["already_loaded"], true);
    assert!(other["selected_resources"].as_array().unwrap().is_empty());
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
