use super::*;
#[tokio::test]
async fn ordinary_multifile_python_project_is_checked_and_delivered_without_html() {
    let patch = "*** Begin Patch\n*** Add File: arithmetic.py\n+def multiply(a, b):\n+    return a * b\n*** Add File: checks.py\n+from arithmetic import multiply\n+assert multiply(6, 7) == 42\n+print('Python project check passed')\n*** Add File: .gitignore\n+__pycache__/\n*** End Patch";
    let f = fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","coding_apply_patch","coding_check","coding_publish"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":patch}}),
        json!({"fixture_workspace_next":"coding_check","fixture_project_arguments":{"name":"unit tests","runtime":"python","entrypoint":"checks.py"}}),
        json!({"fixture_workspace_next":"coding_publish","fixture_project_arguments":{"files":["arithmetic.py","checks.py",".gitignore"]}}),
        answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let check = invocation(&f, &session, &run, 4).await.result.unwrap();
    assert_eq!(check["check"]["status"], "passed");
    assert!(
        check["execution"]["stdout"]
            .as_str()
            .unwrap()
            .contains("Python project check passed")
    );
    let published = invocation(&f, &session, &run, 5).await.result.unwrap();
    assert_eq!(published["artifacts"].as_array().unwrap().len(), 1);
    let card = &published["artifacts"][0];
    assert_eq!(card["filename"], "project.zip");
    assert_eq!(card["project"]["file_count"], 3);
    assert!(card["preview_url"].is_null());
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn invalid_atomic_patch_is_corrected_without_changes_or_execution() {
    let bad = "*** Begin Patch\n*** Update File: missing.html\n@@\n-old\n+new\n*** End Patch";
    let good = "*** Begin Patch\n*** Add File: index.html\n+<!doctype html><h1>正确网站</h1>\n*** End Patch";
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","coding_apply_patch","workspace_publish"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":bad}}),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":good}}),
        json!({"fixture_workspace_next":"workspace_publish","fixture_project_arguments":{"output_files":["index.html"]}}),
        answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let rejected = invocation(&f, &session, &run, 3).await;
    assert_eq!(
        rejected.state,
        exoharness_contracts::invocation::InvocationState::RejectedBeforeExecution
    );
    assert_eq!(rejected.result.unwrap()["code_executed"], false);
    let fixed = invocation(&f, &session, &run, 4).await;
    assert_eq!(fixed.intent.arguments["expected_revision"], 1);
    assert_eq!(fixed.result.unwrap()["revision"], 2);
    assert!(
        f.upstream.requests.lock().unwrap()[3]["messages"]
            .to_string()
            .contains("file does not exist")
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn project_failure_local_patch_check_and_static_preview_use_one_real_oci_workspace() {
    let source = "*** Begin Patch\n*** Add File: index.html\n+<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"assets/style.css\"><h1>项目预览</h1><a href=\"pages/details.html\">二级页面</a><img src=\"assets/cover.svg\"><script src=\"assets/app.js\"></script></html>\n*** Add File: pages/details.html\n+<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"../assets/style.css\"><h1>二级页面</h1><a href=\"../index.html\">返回首页</a><script src=\"../assets/app.js\"></script></html>\n*** Add File: assets/cover.svg\n+<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\"><rect width=\"100\" height=\"100\" fill=\"green\"/></svg>\n*** Add File: assets/style.css\n+body { background: #fafafa; }\n*** Add File: assets/app.js\n+globalThis.projectReady = false;\n*** Add File: checks/check.js\n+const fs=require('node:fs');\n+if (!fs.readFileSync('assets/app.js','utf8').includes('= true')) { console.error('projectReady must be true'); process.exit(1); }\n+console.log('project check passed');\n*** End Patch";
    let fix = "*** Begin Patch\n*** Update File: assets/app.js\n@@\n-globalThis.projectReady = false;\n+globalThis.projectReady = true;\n*** End Patch";
    let check =
        json!({"name":"source assertion","runtime":"javascript","entrypoint":"checks/check.js"});
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","coding_apply_patch","coding_search","coding_check","coding_publish"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":source}}),
        json!({"fixture_workspace_next":"coding_search","fixture_project_arguments":{"query":"projectReady","prefix":"assets"}}),
        json!({"fixture_workspace_next":"coding_check","fixture_project_arguments":check}),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":fix}}),
        json!({"fixture_workspace_next":"coding_check","fixture_project_arguments":check}),
        json!({"fixture_workspace_next":"coding_publish","fixture_project_arguments":{"entrypoint":"index.html","files":["index.html","pages/details.html","assets/style.css","assets/app.js","assets/cover.svg"]}}),
        answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let failed = invocation(&f, &session, &run, 5).await.result.unwrap();
    assert_eq!(failed["check"]["status"], "failed");
    assert_eq!(failed["execution"]["exit_code"], 1);
    assert_eq!(failed["isError"], false);
    assert!(
        failed["execution"]["stderr"]
            .as_str()
            .unwrap()
            .contains("projectReady must be true")
    );
    let passed = invocation(&f, &session, &run, 7).await.result.unwrap();
    assert_eq!(passed["check"]["status"], "passed");
    assert_eq!(failed["workspace_id"], passed["workspace_id"]);
    let publication = invocation(&f, &session, &run, 8).await.result.unwrap();
    assert_eq!(publication["artifacts"].as_array().unwrap().len(), 1);
    let card = &publication["artifacts"][0];
    assert_eq!(card["filename"], "website.zip");
    assert_eq!(card["project"]["file_count"], 5);
    let preview = url::Url::parse(card["preview_url"].as_str().unwrap()).unwrap();
    let router = exoharness_api::media_router(f.tools.artifacts.clone(), 9790);
    let page = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(preview.path())
                .header("host", "127.0.0.1:9790")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let csp = page.headers()["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("sandbox allow-scripts"));
    assert!(!csp.contains("allow-same-origin"));
    assert!(csp.contains("allow-popups allow-popups-to-escape-sandbox"));
    assert!(!csp.contains("allow-forms"));
    assert!(csp.contains("form-action 'none'"));
    if let Ok(prefix) = std::env::var("EXOHARNESS_PREVIEW_CSP_FIXTURE") {
        std::fs::write(
            format!("{prefix}-project.json"),
            json!({"csp":csp}).to_string(),
        )
        .unwrap();
    }
    let resource = preview.join("assets/app.js").unwrap();
    let script = router
        .oneshot(
            Request::builder()
                .uri(resource.path())
                .header("host", "127.0.0.1:9790")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(script.status(), StatusCode::OK);
    assert!(
        script.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("javascript")
    );
    assert_eq!(
        script.into_body().collect().await.unwrap().to_bytes(),
        "globalThis.projectReady = true;\n"
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 9);
        assert_eq!(requests[0]["tools"].as_array().unwrap().len(), 3); // no eager Coding schemas
        assert!(
            requests[5]["messages"]
                .to_string()
                .contains("projectReady must be true")
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn a_check_cannot_fake_final_delivery_and_unauthorized_coding_tools_never_load() {
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["workspace_create","coding_apply_patch","coding_check"]})),
        tool("workspace_create",json!({})),
        json!({"fixture_workspace_next":"coding_apply_patch","fixture_project_arguments":{"patch":"*** Begin Patch\n*** Add File: check.py\n+print('ok')\n*** End Patch"}}),
        json!({"fixture_workspace_next":"coding_check","fixture_project_arguments":{"name":"smoke","runtime":"python","entrypoint":"check.py"}}),
        answer(),
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(
        invocation(&f, &session, &run, 4).await.result.unwrap()["check"]["status"],
        "passed"
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
    let denied = fixture(
        vec![tool(
            "activate_tools",
            json!({"names":["coding_apply_patch"]}),
        )],
        false,
    )
    .await;
    let (_, run) = start(&denied).await;
    assert_eq!(terminal(&denied, &run).await, RunState::Failed);
    assert_eq!(denied.upstream.requests.lock().unwrap().len(), 1);
    denied.tasks.close().await;
    denied.sandbox.stop().await.unwrap();
    denied.server.abort();
}
