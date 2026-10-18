//! Explicit one-request acceptance. Only proposes a plan; never dispatches media.
use exoharness_configuration::Models;
use exoharness_contracts::model::{ChatRequest, ToolChoice};
use exoharness_model_runtime::{CancellationToken, ModelRuntime, RuntimePolicy};
use std::{path::Path, sync::Arc, time::Duration};

#[tokio::test]
#[ignore = "requires explicit private model configuration; incurs one bounded model request"]
async fn one_model_request_authors_all_fifteen_mv_sections() {
    let directory = std::env::var("EXOHARNESS_MV_TEST_MODEL_DIRECTORY")
        .expect("private copied configuration required");
    let path =
        std::env::var("EXOHARNESS_MV_TEST_REQUEST").expect("private projected request required");
    let output = std::env::var("EXOHARNESS_MV_TEST_RESULT").expect("private result path required");
    assert!(
        !Path::new(&output).exists(),
        "never repeat a completed acceptance call"
    );
    let models = Arc::new(Models::open(Path::new(&directory)).unwrap());
    let catalog = models.catalog();
    let name = catalog["default_models"]["llm"]
        .as_str()
        .or_else(|| catalog["default_model"].as_str())
        .map(str::to_owned);
    let name = name.expect("default LLM required");
    let mut request: ChatRequest = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(serde_json::to_vec(&request).unwrap().len() < 128 * 1024);
    let tool = request
        .tools
        .iter()
        .find(|t| t.name.starts_with("mcp_write_music_video_plan_sections_"))
        .expect("only the section authoring stage is exercised")
        .name
        .clone();
    request.tools.retain(|t| t.name == tool);
    request.tool_choice = ToolChoice::Named(tool.clone());
    request.output_token_limit = 16384;
    let runtime = ModelRuntime::new(
        models.clone(),
        RuntimePolicy {
            timeout: Duration::from_secs(180),
            concurrency: 1,
            queued: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let completion = runtime
        .complete_revision(
            &name,
            models.revision(),
            request,
            CancellationToken::default(),
        )
        .await
        .unwrap();
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(output)
            .unwrap();
        std::io::Write::write_all(&mut file, &serde_json::to_vec(&completion).unwrap()).unwrap();
    }
    assert_eq!(completion.calls.len(), 1);
    assert_eq!(completion.calls[0].name, tool);
    let sections = completion.calls[0].arguments["sections"]
        .as_array()
        .unwrap();
    assert_eq!(sections.len(), 15);
    assert!(sections.iter().all(|s| {
        s["content"]["storyboard"]
            .as_array()
            .is_some_and(|shots| shots.len() >= 3)
    }));
    println!(
        "One request, 15 sections, usage {}",
        serde_json::to_string(&completion.usage).unwrap()
    );
    runtime.close().await;
}
