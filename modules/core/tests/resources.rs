use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::{Extension, Host},
    resources::{ManagedTask, OwnedResource},
    services::ExtensionContext,
};
use std::{
    future::pending,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Cleanup {
    name: String,
    log: Arc<Mutex<Vec<String>>>,
    fail_once: bool,
}
#[async_trait]
impl OwnedResource for Cleanup {
    async fn release(&mut self) -> Result<(), String> {
        self.log.lock().unwrap().push(self.name.clone());
        if self.fail_once {
            self.fail_once = false;
            Err("retry cleanup".into())
        } else {
            Ok(())
        }
    }
}
struct Effects {
    log: Arc<Mutex<Vec<String>>>,
    fail_start: bool,
    fail_cleanup: bool,
}
#[async_trait]
impl Extension for Effects {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "effects".into(),
            provides: vec![],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        for name in ["first", "second"] {
            context.own(
                name,
                Box::new(Cleanup {
                    name: name.into(),
                    log: self.log.clone(),
                    fail_once: name == "second" && self.fail_cleanup,
                }),
            )?;
        }
        if self.fail_start {
            Err("failed after allocation".into())
        } else {
            Ok(())
        }
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn failed_setup_releases_scoped_effects_in_reverse_order() {
    let log = Arc::new(Mutex::new(vec![]));
    let mut host = Host::default();
    host.register(Box::new(Effects {
        log: log.clone(),
        fail_start: true,
        fail_cleanup: false,
    }))
    .unwrap();
    assert!(host.start().await.is_err());
    assert_eq!(*log.lock().unwrap(), ["second", "first"]);
    assert!(host.shutdown().await.is_empty());
    assert_eq!(log.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn failed_cleanup_remains_owned_and_successfully_released_effects_are_not_replayed() {
    let log = Arc::new(Mutex::new(vec![]));
    let mut host = Host::default();
    host.register(Box::new(Effects {
        log: log.clone(),
        fail_start: false,
        fail_cleanup: true,
    }))
    .unwrap();
    host.start().await.unwrap();
    assert_eq!(host.shutdown().await.len(), 1);
    assert_eq!(*log.lock().unwrap(), ["second"]);
    assert!(host.shutdown().await.is_empty());
    assert_eq!(*log.lock().unwrap(), ["second", "second", "first"]);
    host.shutdown().await;
    assert_eq!(log.lock().unwrap().len(), 3);
}

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn managed_task_cleanup_is_awaited_and_drop_has_an_abort_safety_net() {
    for explicit in [true, false] {
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = dropped.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _owned = Dropped(flag);
            tx.send(()).unwrap();
            pending::<()>().await;
        });
        rx.await.unwrap();
        let mut task = ManagedTask::new(task);
        if explicit {
            task.release().await.unwrap();
            assert!(dropped.load(Ordering::SeqCst));
            task.release().await.unwrap();
        }
        drop(task);
        tokio::task::yield_now().await;
        assert!(dropped.load(Ordering::SeqCst));
    }
}
