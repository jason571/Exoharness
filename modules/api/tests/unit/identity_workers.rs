//! Worker saturation must not turn ordinary page refresh into a rate-limit error.
use super::work;
use exoharness_identity::{AuthStore, Database};
use std::sync::{Arc, Condvar, Mutex};
#[tokio::test]
async fn ordinary_requests_wait_and_cancelled_queue_entries_do_not_take_workers() {
    let store = tokio::task::spawn_blocking(|| {
        let url =
            std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated database required");
        Arc::new(
            AuthStore::open(
                Database::connect(
                    &url,
                    None,
                    &format!("pro_identity_workers_{}", std::process::id()),
                )
                .unwrap(),
            )
            .unwrap(),
        )
    })
    .await
    .unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut owners = Vec::new();
    for _ in 0..4 {
        let gate = gate.clone();
        let started = started_tx.clone();
        let store = store.clone();
        owners.push(tokio::spawn(async move {
            work(store, move |_| {
                started.send(()).unwrap();
                let (lock, signal) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = signal.wait(released).unwrap();
                }
                Ok(())
            })
            .await
        }));
    }
    for _ in 0..4 {
        started_rx.recv().await.unwrap();
    }
    let queued = tokio::spawn({
        let store = store.clone();
        async move { work(store, |_| Ok(7)).await }
    });
    let cancelled = tokio::spawn({
        let store = store.clone();
        async move { work(store, |_| Ok(8)).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let waited = !queued.is_finished();
    cancelled.abort();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    for owner in owners {
        assert!(owner.await.unwrap().is_ok());
    }
    let result = queued.await.unwrap();
    assert!(
        waited,
        "normal read must queue, not be rejected merely because four workers are active"
    );
    assert_eq!(result.map_err(|_| "identity worker failed").unwrap(), 7);
    let mut requests = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        requests.push(tokio::spawn(async move { work(store, |_| Ok(true)).await }));
    }
    for request in requests {
        assert!(request.await.unwrap().is_ok_and(|v| v));
    }
}
