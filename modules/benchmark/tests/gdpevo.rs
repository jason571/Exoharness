use exoharness_benchmark::gdpevo::{Dataset, Gateway, Grader};
use serde_json::json;
use std::path::Path;
fn dataset(root: &Path) -> Dataset {
    let group = root.join("data/task_groups/task_group_001");
    std::fs::create_dir_all(group.join("env")).unwrap();
    std::fs::write(
        group.join("env/endpoints.txt"),
        "GET /api/data\nGET /api/items/{id}\nPOST /api/judge\nPOST /api/operator/reset\n",
    )
    .unwrap();
    std::fs::write(
        group.join("task_group.yaml"),
        "env:\n  state_mode: read_only\n",
    )
    .unwrap();
    for split in ["train", "test"] {
        let task = group.join(format!("{split}_tasks/001"));
        for name in ["input", "output", "eval", "notes"] {
            std::fs::create_dir_all(task.join(name)).unwrap();
        }
        std::fs::write(task.join("input/prompt.txt"), format!("Solve {split}")).unwrap();
        std::fs::write(
            task.join("output/answer.json"),
            "{\"private\":\"GOLD_SECRET\"}",
        )
        .unwrap();
        std::fs::write(task.join("notes/private.txt"), "PRIVATE NOTES").unwrap();
        std::fs::write(task.join("eval/eval.sh"), "echo PRIVATE JUDGE").unwrap();
    }
    for args in [
        vec!["init", "-q"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let revision = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    Dataset::new(root, String::from_utf8_lossy(&revision.stdout).trim()).unwrap()
}
#[test]
fn frozen_data_and_solver_input_never_mix_with_grading_sources() {
    let temp = tempfile::tempdir().unwrap();
    let d = dataset(temp.path());
    let input = d
        .solver_input("task_group_001", "test", "001", "http://127.0.0.1:19999")
        .unwrap();
    assert_eq!(input["prompt"], "Solve test");
    assert!(!input.to_string().contains("GOLD_SECRET"));
    assert!(!input.to_string().contains("PRIVATE"));
    let inventory = d.inventory().unwrap();
    assert_eq!(inventory["tasks"].as_array().unwrap().len(), 2);
    Dataset::from_snapshot(temp.path(), &d.revision, &inventory).unwrap();
    for (group, split, task) in [
        ("../escape", "test", "001"),
        ("task_group_001", "validation", "001"),
        ("task_group_001", "test", "../001"),
    ] {
        assert!(d.path(group, split, task).is_err());
    }
    std::fs::write(
        d.path("task_group_001", "test", "001")
            .unwrap()
            .join("input/prompt.txt"),
        "changed",
    )
    .unwrap();
    assert!(Dataset::from_snapshot(temp.path(), &d.revision, &inventory).is_err());
    assert!(Dataset::new(temp.path(), &d.revision).is_err());
}
#[tokio::test]
async fn domain_gateway_only_forwards_declared_routes_and_no_credentials() {
    use axum::{Router, extract::Request, response::IntoResponse, routing::get};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let temp = tempfile::tempdir().unwrap();
    let d = dataset(temp.path());
    let leaked = Arc::new(AtomicBool::new(false));
    let flag = leaked.clone();
    let upstream = Router::new().route(
        "/api/data",
        get(move |request: Request| {
            let flag = flag.clone();
            async move {
                flag.store(
                    request.headers().get("Authorization").is_some()
                        || request.headers().get("Cookie").is_some(),
                    Ordering::Relaxed,
                );
                axum::Json(json!({"ok":true})).into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let gateway = Gateway::start(&d, "task_group_001", &origin, 0)
        .await
        .unwrap();
    let client = reqwest::Client::new();
    let good = client
        .get(format!("{}/api/data", gateway.origin))
        .header("Authorization", "Bearer NEVER_FORWARD")
        .header("Cookie", "NEVER_FORWARD")
        .send()
        .await
        .unwrap();
    assert_eq!(good.status().as_u16(), 200);
    assert!(!leaked.load(Ordering::Relaxed));
    for path in [
        "/api/judge",
        "/api/operator/reset",
        "/judge",
        "/api/%252e%252e/judge",
        "/api/items/../../judge",
    ] {
        let response = client
            .post(format!("{}{path}", gateway.origin))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 403, "{path}");
    }
    drop(gateway);
    task.abort();
}
#[test]
fn dataset_links_and_unpinned_grading_images_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let d = dataset(temp.path());
    for image in [
        "python:latest",
        "python@sha256:bad",
        "image;touch /tmp/escape",
    ] {
        assert!(Grader::new(d.clone(), image).is_err());
    }
    #[cfg(unix)]
    {
        let path = d
            .path("task_group_001", "test", "001")
            .unwrap()
            .join("input/link");
        std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
        assert!(
            d.solver_input("task_group_001", "test", "001", "http://127.0.0.1:19999")
                .is_err()
        );
    }
}
