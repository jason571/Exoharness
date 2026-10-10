//! Public build identity, not filesystem/configuration disclosure.
use axum::Json;
use serde_json::{Value, json};
pub(crate) async fn info() -> Json<Value> {
    Json(json!({"name":"ExoHarness","version":env!("CARGO_PKG_VERSION"),"product_ready":false}))
}
