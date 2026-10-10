// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Public build identity, not filesystem/configuration disclosure.
use axum::Json;
use serde_json::{Value, json};
pub(crate) async fn info() -> Json<Value> {
    Json(json!({"name":"ExoHarness","version":env!("CARGO_PKG_VERSION"),"product_ready":false}))
}
