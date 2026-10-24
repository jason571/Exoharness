// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("ExoHarness Benchmark {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.is_empty() || args == ["--help"] {
        println!(
            "exoharness-benchmark suite|plan|run|compare|accept|review|gdpevo|validate [options]\nNative Rust evaluation; no Python runtime. Use --allow-model-requests to execute real tasks."
        );
        return;
    }
    match exoharness_benchmark::cli::main(&args).await {
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("JSON result")
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
