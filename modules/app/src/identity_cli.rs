// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Local OS-authorized recovery. Secrets arrive via stdin, not arguments or logs.
use exoharness_identity::{AuthRole, AuthStore, Database};
use std::io::Read;
pub fn reset_admin() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .take(4097)
        .read_to_string(&mut input)
        .map_err(|_| "cannot read password input")?;
    if input.len() > 4096 {
        return Err("password input is too large".into());
    }
    let value =
        exoharness_contracts::strict_json::parse(&input).map_err(|_| "invalid password input")?;
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or("password input must contain exactly one field")?;
    let password = object
        .get("password")
        .and_then(serde_json::Value::as_str)
        .ok_or("password must be text")?;
    let store = AuthStore::open(
        Database::from_env("auth").map_err(|_| "local identity database connection failed")?,
    )
    .map_err(|_| "identity setup failed")?;
    store
        .set_password(AuthRole::Admin, password)
        .map_err(|error| match error {
            exoharness_identity::AuthError::Validation(message) => message,
            _ => "administrator password recovery failed".into(),
        })?;
    println!("Administrator password updated; previous administrator sessions revoked.");
    Ok(())
}
