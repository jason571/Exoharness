//! Operator-only reconciliation of an already declared artifact. No tools or
//! models are called and no chat/journal state is modified by this utility.
use exoharness_artifacts::{Artifacts, RemoteFile};
use exoharness_storage_postgres::PostgresStore;
use serde_json::{Value, json};
use std::{io::Read, path::PathBuf, sync::Arc};
#[tokio::main]
async fn main() {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).expect("input");
    let input: Value = serde_json::from_str(&raw).expect("input JSON");
    let database_namespace = input["namespace"].as_str().expect("namespace").to_owned();
    let root = PathBuf::from(input["root"].as_str().expect("root"));
    let base = input["public_base"]
        .as_str()
        .expect("public_base")
        .to_owned();
    let service = tokio::task::spawn_blocking(move || {
        let store = PostgresStore::from_env(&database_namespace).expect("database access");
        Arc::new(Artifacts::open(root, store, &base).expect("artifact service"))
    })
    .await
    .expect("blocking worker");
    let published = service
        .import_remote(
            input["owner"].as_str().expect("owner"),
            input["session"].as_str().expect("session"),
            input["run"].as_str().expect("run"),
            RemoteFile {
                service_url: input["service_url"]
                    .as_str()
                    .expect("authorized service_url"),
                url: input["url"].as_str().expect("declared URL"),
                filename: input["filename"].as_str().expect("filename"),
                size: input["size_bytes"].as_u64(),
                sha256: input["sha256"].as_str(),
            },
        )
        .await
        .expect("declared output import");
    let mut card = serde_json::to_value(published.artifact).expect("card");
    card["kind"] = json!(match card["content_type"]
        .as_str()
        .unwrap_or("")
        .split('/')
        .next()
    {
        Some("audio") => "audio",
        Some("video") => "video",
        Some("image") => "image",
        _ => "file",
    });
    card["url"] = json!(published.url);
    card["download_url"] = json!(published.download_url);
    println!("{}", card);
}
