use std::process::Command;

#[test]
fn version_runs_without_database_or_credentials() {
    let output = Command::new(env!("CARGO_BIN_EXE_exoharness"))
        .arg("--version")
        .env_clear()
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("ExoHarness ")
    );
}

#[test]
fn unexpected_arguments_fail_before_service_startup() {
    let output = Command::new(env!("CARGO_BIN_EXE_exoharness"))
        .arg("--not-supported")
        .env_clear()
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
