use super::tool_trace::{ADMIN, get};
use super::*;

#[tokio::test]
async fn alternating_unchanged_source_reads_replan_once_then_publish_without_public_errors() {
    let mut replies = vec![
        tool(
            "activate_tools",
            json!({"names":["workspace_create","workspace_edit","workspace_read","workspace_publish"]}),
        ),
        tool("workspace_create", json!({})),
        json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":{"index.html":"<!doctype html><html><h1>ExoHarness</h1></html>","script.js":"const ready=true;","check.py":"print('ok')"}}),
    ];
    for round in 0..2 {
        for path in ["index.html", "script.js", "check.py"] {
            let arguments = if round == 0 {
                json!({"path":path,"offset":0,"limit":16000})
            } else {
                json!({"path":path})
            };
            replies.push(json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":arguments}));
        }
    }
    replies.push(json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":{"path":"index.html","offset":0,"limit":16000}}));
    replies.push(json!({"fixture_workspace_next":"workspace_publish"}));
    replies.push(answer());
    let f = fixture(replies, true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let router = super::tool_trace::management(&f, Surface::Management);
    let (_, trace) = get(
        &router,
        &format!("/v1/trace/runs/{run}?event_limit=500"),
        ADMIN,
    )
    .await;
    let warnings: Vec<_> = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["payload"]["source"] == "read_loop")
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["payload"]["proposal_not_submitted"], true);
    assert_eq!(
        trace["invocations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["tool_name"] == "workspace_read")
            .count(),
        6
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 12);
    assert!(f.upstream.replies.lock().unwrap().is_empty());
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/sessions/{session}"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let history: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        history["turns"][0]["response"]["artifacts"][0]["filename"],
        "index.html"
    );
    let text = serde_json::to_string(&history).unwrap();
    assert!(!text.contains("repeated_unchanged_read"));
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert!(
            requests[10]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Host read-loop review")
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn modified_source_supersedes_old_read_with_digest_verified_current_text_then_publishes() {
    let current = "<!doctype html><html><h1>CURRENT-SOURCE</h1></html>".to_owned()
        + &"<!--NEW-->".repeat(300);
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","workspace_edit","workspace_read","workspace_publish"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"workspace_edit"}),
        json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":{"path":"index.html","limit":16000}}),
        json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":{"index.html":current}}),
        json!({"fixture_workspace_next":"workspace_publish"}),answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        let projected = requests[5]["messages"].to_string();
        assert!(projected.contains("current_authored_sources"));
        assert!(projected.contains("CURRENT-SOURCE"));
        assert!(!projected.contains("fixture-large-layout"));
        assert!(projected.contains("read_body_superseded_by"));
    }
    let original = invocation(&f, &session, &run, 4).await.result.unwrap();
    assert!(
        original["content"]
            .as_str()
            .unwrap()
            .contains("fixture-large-layout")
    );
    assert_eq!(
        invocation(&f, &session, &run, 6).await.result.unwrap()["artifacts"][0]["filename"],
        "index.html"
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn shrinking_same_anchor_queries_replan_with_verified_excerpt_without_a_third_file_read() {
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","workspace_edit","workspace_read","workspace_publish"]})),tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"workspace_edit","fixture_workspace_files":{"index.html":"<!doctype html><html>published</html>","check.py":"HEAD\nneedle exact code and remaining source"}}),
        json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":{"path":"check.py","query":"needle","offset":0,"limit":16000}}),
        json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":{"path":"check.py","query":"needle","offset":1,"limit":15}}),
        json!({"fixture_workspace_next":"workspace_read","fixture_project_arguments":{"path":"check.py","query":"needle","offset":2,"limit":6}}),
        json!({"fixture_workspace_next":"workspace_publish"}),answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let router = super::tool_trace::management(&f, Surface::Management);
    let (_, trace) = get(
        &router,
        &format!("/v1/trace/runs/{run}?event_limit=500"),
        ADMIN,
    )
    .await;
    assert_eq!(
        trace["invocations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["tool_name"] == "workspace_read")
            .count(),
        2
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 8);
        let replan = requests[6]["messages"].to_string();
        assert!(replan.contains("requested_excerpt"));
        assert!(replan.contains("needle"));
    }
    assert_eq!(
        invocation(&f, &session, &run, 7).await.result.unwrap()["artifacts"][0]["filename"],
        "index.html"
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
