use exoharness_api::{ChatTasks, TaskAdmissionError};
use exoharness_model_runtime::CancellationToken;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
struct Dropped(Arc<AtomicBool>);

#[tokio::test]
async fn default_capacity_admits_the_maximum_global_limit_with_queue_headroom() {
    let tasks = ChatTasks::default();
    for index in 0..1025 {
        let token = CancellationToken::new();
        let cancelled = token.clone();
        tasks
            .spawn(format!("capacity-{index}"), token, async move {
                cancelled.cancelled().await;
            })
            .await
            .unwrap();
    }
    assert_eq!(tasks.close().await.len(), 1025);
}
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn shutdown_cancels_joins_and_both_callers_wait_for_cleanup() {
    let tasks = Arc::new(ChatTasks::new(2, Duration::from_secs(1)).unwrap());
    let token = CancellationToken::new();
    let cancelled = token.clone();
    let clean = Arc::new(AtomicBool::new(false));
    let observed = clean.clone();
    tasks
        .spawn("run-1".into(), token, async move {
            cancelled.cancelled().await;
            tokio::time::sleep(Duration::from_millis(20)).await;
            observed.store(true, Ordering::SeqCst);
        })
        .await
        .unwrap();
    let (a, b) = tokio::join!(tasks.close(), tasks.close());
    assert_eq!(a.len() + b.len(), 1);
    assert!(clean.load(Ordering::SeqCst));
    assert!(!tasks.is_open().await);
    assert_eq!(
        tasks
            .spawn("run-2".into(), CancellationToken::new(), async {})
            .await,
        Err(TaskAdmissionError::Closed)
    );
}
#[tokio::test]
async fn stalled_worker_is_aborted_and_dropped_before_close_returns() {
    let tasks = ChatTasks::new(1, Duration::from_millis(30)).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(dropped.clone());
    tasks
        .spawn("stalled".into(), CancellationToken::new(), async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        })
        .await
        .unwrap();
    assert_eq!(tasks.close().await, vec!["stalled"]);
    assert!(dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn interrupted_cleanup_retains_handles_for_the_next_drain() {
    let tasks = ChatTasks::new(1, Duration::from_secs(1)).unwrap();
    let release = Arc::new(Notify::new());
    let worker_release = release.clone();
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Dropped(dropped.clone());
    tasks
        .spawn("retained".into(), CancellationToken::new(), async move {
            let _guard = guard;
            worker_release.notified().await;
        })
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(5), tasks.close())
            .await
            .is_err()
    );
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(!tasks.is_open().await);
    release.notify_one();
    assert_eq!(tasks.close().await, vec!["retained"]);
    assert!(dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn duplicate_and_capacity_reject_without_polling_proposed_workers() {
    let tasks = ChatTasks::new(1, Duration::from_millis(20)).unwrap();
    tasks
        .spawn(
            "one".into(),
            CancellationToken::new(),
            std::future::pending(),
        )
        .await
        .unwrap();
    let called = Arc::new(AtomicBool::new(false));
    let first = called.clone();
    assert_eq!(
        tasks
            .spawn("one".into(), CancellationToken::new(), async move {
                first.store(true, Ordering::SeqCst);
            })
            .await,
        Err(TaskAdmissionError::Duplicate)
    );
    let second = called.clone();
    assert_eq!(
        tasks
            .spawn("two".into(), CancellationToken::new(), async move {
                second.store(true, Ordering::SeqCst);
            })
            .await,
        Err(TaskAdmissionError::Capacity)
    );
    assert!(!called.load(Ordering::SeqCst));
    tasks.close().await;
}
