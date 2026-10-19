use super::*;

#[tokio::test]
async fn invalid_model_tool_is_corrected_once_before_real_oci_and_failed_usage_is_exact() {
    let f=fixture(vec![
        tool("sandbox_run",json!({})),
        tool("activate_tools",json!({"names":["sandbox_run"]})),
        tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"from pathlib import Path\nPath('/work/final.txt').write_text('corrected once')\n"},"output_files":["final.txt"]})),answer()
    ],true).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let (_, trace) = tool_trace::get(
        &tool_trace::management(&f, Surface::Management),
        &format!("/v1/trace/runs/{run}?event_limit=100"),
        tool_trace::ADMIN,
    )
    .await;
    assert_eq!(trace["model_calls"].as_array().unwrap().len(), 4);
    assert_eq!(trace["invocations"].as_array().unwrap().len(), 2);
    assert_eq!(trace["model_calls"][0]["status"], "failed");
    assert_eq!(trace["model_calls"][0]["provider"], "test");
    assert_eq!(
        trace["model_calls"][0]["output"]["model_error"]["diagnostic"],
        "tool_name_not_loaded"
    );
    let usage = trace["model_calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["usage"]["total_tokens"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(usage, 450);
    assert_eq!(
        trace["invocations"][1]["result"]["execution"]["exit_code"],
        0
    );
    assert!(
        f.upstream.requests.lock().unwrap()[1]
            .to_string()
            .contains("outside the currently loaded schemas")
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn repeated_invalid_or_unmeasured_proposals_stop_without_tools_or_hidden_requests() {
    for unknown_usage in [false, true] {
        let mut invalid = tool("not_loaded", json!({}));
        if unknown_usage {
            invalid["usage"] = Value::Null;
        }
        let replies = if unknown_usage {
            vec![invalid]
        } else {
            vec![invalid.clone(), invalid]
        };
        let f = fixture(replies, true).await;
        let (_, run) = start(&f).await;
        assert_eq!(terminal(&f, &run).await, RunState::Failed);
        let (_, trace) = tool_trace::get(
            &tool_trace::management(&f, Surface::Management),
            &format!("/v1/trace/runs/{run}"),
            tool_trace::ADMIN,
        )
        .await;
        assert!(trace["invocations"].as_array().unwrap().is_empty());
        assert_eq!(
            f.upstream.requests.lock().unwrap().len(),
            if unknown_usage { 1 } else { 2 }
        );
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}
