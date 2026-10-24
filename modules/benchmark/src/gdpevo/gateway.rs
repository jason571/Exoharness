// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use super::Dataset;
use crate::{
    Result, ensure,
    harness::{bounded, origin},
};
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::{sync::Arc, time::Duration};
struct Routes {
    upstream: String,
    routes: Vec<(String, regex::Regex)>,
    client: reqwest::Client,
}
pub struct Gateway {
    pub origin: String,
    state: Arc<Routes>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Gateway {
    pub async fn start(dataset: &Dataset, group: &str, upstream: &str, port: u16) -> Result<Self> {
        dataset.path(group, "test", "001")?;
        let upstream = origin(upstream)?;
        let raw =
            String::from_utf8(dataset.read(&dataset.groups.join(group).join("env/endpoints.txt"))?)
                .map_err(|_| crate::Error::Invalid("Invalid domain route declaration".into()))?;
        let declaration = regex::Regex::new(r"^(GET|POST)\s+(/[^\s]*)$").expect("pattern");
        let denied =
            regex::Regex::new("(?i)judge|eval|gold|answer|reset|operator").expect("pattern");
        let parameters = regex::Regex::new(r"\{[^{}]+\}|<[^<>]+>").expect("pattern");
        let mut routes = Vec::new();
        for line in raw.lines() {
            if let Some(m) = declaration.captures(line.trim()) {
                let path = m[2].split('?').next().unwrap_or("");
                if denied.is_match(path) {
                    continue;
                }
                let mut pattern = String::from("^");
                let mut offset = 0;
                for part in parameters.find_iter(path) {
                    pattern.push_str(&regex::escape(&path[offset..part.start()]));
                    pattern.push_str("[A-Za-z0-9_.-]+");
                    offset = part.end();
                }
                pattern.push_str(&regex::escape(&path[offset..]));
                pattern.push('$');
                routes.push((
                    m[1].to_string(),
                    regex::Regex::new(&pattern)
                        .map_err(|_| crate::Error::Invalid("Invalid route pattern".into()))?,
                ));
            }
        }
        ensure(!routes.is_empty(), "No released domain routes")?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(Duration::from_secs(20))
            .build()?;
        let state = Arc::new(Routes {
            upstream,
            routes,
            client,
        });
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
        let app = Router::new().fallback(forward).with_state(state.clone());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        Ok(Self {
            origin,
            state,
            shutdown: Some(tx),
            task,
        })
    }
    pub async fn reset_owned(&self) -> Result<()> {
        let response = self
            .state
            .client
            .post(format!("{}/api/operator/reset", self.state.upstream))
            .json(&serde_json::json!({}))
            .send()
            .await?;
        ensure(
            response.status().is_success(),
            "Owned environment reset failed",
        )?;
        bounded(response, 1024 * 1024).await?;
        Ok(())
    }
}
impl Drop for Gateway {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        self.task.abort();
    }
}
async fn forward(State(state): State<Arc<Routes>>, request: Request) -> Response {
    let method = request.method().to_string();
    let raw = request.uri().to_string();
    let decoded = match percent_encoding::percent_decode_str(&raw).decode_utf8() {
        Ok(s) => s,
        Err(_) => return StatusCode::FORBIDDEN.into_response(),
    };
    let path = decoded.split('?').next().unwrap_or("");
    if !raw.starts_with('/')
        || raw.starts_with("//")
        || decoded.contains(['\\', '%'])
        || path.split('/').any(|p| p == "..")
        || !state
            .routes
            .iter()
            .any(|(m, p)| m == &method && p.is_match(path))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let body = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(v) => v,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let result = async {
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| crate::Error::Invalid("Invalid domain method".into()))?;
        let mut call = state
            .client
            .request(method, format!("{}{raw}", state.upstream))
            .header("Content-Type", "application/json");
        if !body.is_empty() {
            call = call.body(body);
        }
        let response = call.send().await?;
        ensure(response.status().is_success(), "Domain service unavailable")?;
        let content_type = response.headers().get("Content-Type").cloned();
        let data = bounded(response, 8 * 1024 * 1024).await?;
        let mut response = Response::new(Body::from(data));
        if let Some(content_type) = content_type {
            response.headers_mut().insert("Content-Type", content_type);
        }
        Ok::<_, crate::Error>(response)
    }
    .await;
    result.unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}
