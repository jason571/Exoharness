// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        println!("ExoHarness Evolution {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.is_empty() || args == ["--help"] {
        println!(
            "exoharness-evolution run|publish|monitor|watch|status|skill-package|validate [options]\nNative Rust Skill evolution; no Python runtime."
        );
        return;
    }
    match exoharness_evolution::cli::main(&args).await {
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
