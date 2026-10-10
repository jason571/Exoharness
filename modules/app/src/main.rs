// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

mod artifacts_extension;
mod chat_extension;
mod config;
mod extensions;
mod identity_cli;
mod mcp_extension;
mod model_extension;
mod sandbox_extension;
mod server;
mod skill_runtime_extension;
mod skills_extension;
mod tools_extension;

#[tokio::main]
async fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments == ["--set-local-admin-password"] {
        if let Err(error) = identity_cli::reset_admin() {
            eprintln!("Local password recovery failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments == ["--version"] {
        println!(
            "ExoHarness {} (local infrastructure validation)",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    if arguments.len() == 2 && arguments[0] == "--check-configuration" {
        match exoharness_configuration::check_directory(std::path::Path::new(&arguments[1])) {
            Ok(report) => println!("{report}"),
            Err(error) => {
                eprintln!("Configuration check failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if !arguments.is_empty() {
        eprintln!(
            "Usage: exoharness [--version]; configure the service with EXOHARNESS_* environment variables"
        );
        std::process::exit(2);
    }
    if let Err(error) = server::run().await {
        eprintln!("ExoHarness local validation stopped: {error}");
        std::process::exit(1);
    }
}
