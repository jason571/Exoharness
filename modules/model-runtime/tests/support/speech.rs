//! Local speech protocol fixture, never a real upstream or secret.
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Response, StatusCode},
    routing::{get, post},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
    pub delay: Duration,
}
impl Reply {
    pub fn json(text: &str) -> Self {
        Self {
            status: StatusCode::OK,
            content_type: "application/json",
            bytes: text.as_bytes().to_vec(),
            delay: Duration::ZERO,
        }
    }
    pub fn sse(text: &str) -> Self {
        Self {
            content_type: "text/event-stream",
            ..Self::json(text)
        }
    }
}
pub struct Data {
    pub reply: Mutex<Reply>,
    pub received: Mutex<Vec<(HeaderMap, Vec<u8>)>>,
    count: AtomicUsize,
}
pub struct Server {
    pub base: String,
    pub state: Arc<Data>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    pub async fn new(reply: Reply) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Data {
            reply: Mutex::new(reply),
            received: Mutex::new(vec![]),
            count: AtomicUsize::new(0),
        });
        let router = Router::new()
            .route("/v1/models", get(handle))
            .route("/v1/audio/transcriptions", post(handle))
            .route("/synthesize", post(handle))
            .with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { base, state, task }
    }
    pub fn count(&self) -> usize {
        self.state.count.load(Ordering::SeqCst)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort()
    }
}
async fn handle(State(state): State<Arc<Data>>, headers: HeaderMap, body: Bytes) -> Response<Body> {
    state.count.fetch_add(1, Ordering::SeqCst);
    state
        .received
        .lock()
        .unwrap()
        .push((headers, body.to_vec()));
    let reply = state.reply.lock().unwrap().clone();
    tokio::time::sleep(reply.delay).await;
    let chunks = reply
        .bytes
        .chunks(3)
        .map(|part| Ok::<_, std::io::Error>(Bytes::copy_from_slice(part)))
        .collect::<Vec<_>>();
    Response::builder()
        .status(reply.status)
        .header("content-type", reply.content_type)
        .body(Body::from_stream(tokio_stream::iter(chunks)))
        .unwrap()
}
