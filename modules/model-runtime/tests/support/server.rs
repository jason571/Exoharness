//! Controllable local protocol server. No real model/key or production network.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Response, StatusCode},
    routing::post,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
pub struct Reply {
    pub status: StatusCode,
    pub body: String,
    pub content_type: &'static str,
    pub delay: Duration,
    pub retry_after: Option<&'static str>,
    pub chunked: bool,
}
impl Reply {
    pub fn json(value: Value) -> Self {
        Self {
            status: StatusCode::OK,
            body: value.to_string(),
            content_type: "application/json",
            delay: Duration::ZERO,
            retry_after: None,
            chunked: false,
        }
    }
}
pub struct StateData {
    pub reply: Mutex<Reply>,
    pub count: AtomicUsize,
    pub received: Mutex<Vec<(HeaderMap, Value)>>,
}
pub struct Server {
    pub url: String,
    pub state: Arc<StateData>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn new(reply: Reply) -> Self {
        let state = Arc::new(StateData {
            reply: Mutex::new(reply),
            count: AtomicUsize::new(0),
            received: Mutex::new(Vec::new()),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let router = Router::new()
            .route("/v1/chat/completions", post(handle))
            .route("/v1/messages", post(handle))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { url, state, task }
    }
    pub fn count(&self) -> usize {
        self.state.count.load(Ordering::SeqCst)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn handle(
    State(state): State<Arc<StateData>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    state.count.fetch_add(1, Ordering::SeqCst);
    state
        .received
        .lock()
        .unwrap()
        .push((headers, serde_json::from_slice(&body).unwrap()));
    let (status, body, content_type, delay, retry_after, chunked) = {
        let reply = state.reply.lock().unwrap();
        (
            reply.status,
            reply.body.clone(),
            reply.content_type,
            reply.delay,
            reply.retry_after,
            reply.chunked,
        )
    };
    tokio::time::sleep(delay).await;
    let mut response = Response::builder()
        .status(status)
        .header("content-type", content_type);
    if let Some(retry_after) = retry_after {
        response = response.header("retry-after", retry_after);
    }
    let body = if chunked {
        let chunks = body
            .into_bytes()
            .chunks(31)
            .map(|chunk| Ok::<_, std::io::Error>(Bytes::copy_from_slice(chunk)))
            .collect::<Vec<_>>();
        if content_type == "text/event-stream" {
            // Deliberately pace each wire chunk. Tests must observe a delta
            // before the upstream EOF, not just split a buffered JSON body.
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                for chunk in chunks {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    if tx.send(chunk).await.is_err() {
                        break;
                    }
                }
            });
            Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
        } else {
            Body::from_stream(tokio_stream::iter(chunks))
        }
    } else {
        Body::from(body)
    };
    response.body(body).unwrap()
}
pub fn answer() -> Value {
    json!({"id":"test-response","choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":4}}})
}
