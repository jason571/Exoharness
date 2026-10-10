use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::{Extension, Host, HostError, HostState},
    services::{ExtensionContext, Service},
};
use std::{
    future::{Future, pending},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

struct JournalService;
impl Service for JournalService {
    const KEY: &'static str = "journal/v1";
}
struct WrongType;
impl Service for WrongType {
    const KEY: &'static str = "journal/v1";
}
type Log = Arc<Mutex<Vec<String>>>;

struct Fixture {
    spec: ExtensionSpec,
    log: Log,
    fail_start: bool,
    fail_stop_once: bool,
    pending_start: bool,
    pending_stop_once: bool,
    skip_publish: bool,
    undeclared_read: bool,
}

fn fixture(id: &str, provider: bool, consumer: bool, log: &Log) -> Fixture {
    Fixture {
        spec: ExtensionSpec {
            id: id.into(),
            provides: if provider {
                vec![JournalService::KEY.into()]
            } else {
                vec![]
            },
            requires: if consumer {
                vec![JournalService::KEY.into()]
            } else {
                vec![]
            },
        },
        log: log.clone(),
        fail_start: false,
        fail_stop_once: false,
        pending_start: false,
        pending_stop_once: false,
        skip_publish: false,
        undeclared_read: false,
    }
}

#[async_trait]
impl Extension for Fixture {
    fn spec(&self) -> ExtensionSpec {
        self.spec.clone()
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        self.log
            .lock()
            .unwrap()
            .push(format!("start:{}", self.spec.id));
        if !self.spec.provides.is_empty() && !self.skip_publish {
            context
                .provide(Arc::new(JournalService))
                .map_err(|e| e.to_string())?;
        }
        if !self.spec.requires.is_empty() || self.undeclared_read {
            context
                .require::<JournalService>()
                .map_err(|e| e.to_string())?;
        }
        if self.pending_start {
            pending::<()>().await;
        }
        if self.fail_start {
            Err("setup failed".into())
        } else {
            Ok(())
        }
    }
    async fn stop(&mut self) -> Result<(), String> {
        self.log
            .lock()
            .unwrap()
            .push(format!("stop:{}", self.spec.id));
        if self.pending_stop_once {
            self.pending_stop_once = false;
            pending::<()>().await;
        }
        if self.fail_stop_once {
            self.fail_stop_once = false;
            Err("cleanup failed".into())
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn declared_typed_dependency_is_available_only_after_host_readiness() {
    let log = Log::default();
    let mut host = Host::default();
    host.register(Box::new(fixture("agent", false, true, &log)))
        .unwrap();
    host.register(Box::new(fixture("storage", true, false, &log)))
        .unwrap();
    assert!(host.service::<JournalService>().is_err());
    host.start().await.unwrap();
    assert!(host.service::<JournalService>().is_ok());
    assert!(host.service::<WrongType>().is_err());
    assert!(
        host.register(Box::new(fixture("late", false, false, &log)))
            .is_err()
    );
    assert!(host.start().await.is_err());
    assert!(host.shutdown().await.is_empty());
    assert_eq!(
        *log.lock().unwrap(),
        ["start:storage", "start:agent", "stop:agent", "stop:storage"]
    );
    assert!(host.service::<JournalService>().is_err());
    assert!(host.shutdown().await.is_empty());
    assert_eq!(log.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn failed_start_rolls_back_partial_extension_then_dependencies() {
    let log = Log::default();
    let mut host = Host::default();
    let mut agent = fixture("agent", false, true, &log);
    agent.fail_start = true;
    host.register(Box::new(agent)).unwrap();
    host.register(Box::new(fixture("storage", true, false, &log)))
        .unwrap();
    assert!(matches!(host.start().await, Err(HostError::Start { .. })));
    assert_eq!(host.state(), HostState::Failed);
    assert_eq!(
        *log.lock().unwrap(),
        ["start:storage", "start:agent", "stop:agent", "stop:storage"]
    );
    assert!(host.service::<JournalService>().is_err());
}

#[tokio::test]
async fn missing_dependency_has_zero_initialization_side_effects() {
    let log = Log::default();
    let mut host = Host::default();
    host.register(Box::new(fixture("agent", false, true, &log)))
        .unwrap();
    assert!(matches!(host.start().await, Err(HostError::Graph(_))));
    assert_eq!(host.state(), HostState::Idle);
    assert!(log.lock().unwrap().is_empty());
}

#[tokio::test]
async fn undeclared_reads_and_missing_publications_fail_and_release_partial_setup() {
    for undeclared_read in [false, true] {
        let log = Log::default();
        let mut host = Host::default();
        let mut f = fixture("extension", !undeclared_read, false, &log);
        f.skip_publish = true;
        f.undeclared_read = undeclared_read;
        host.register(Box::new(f)).unwrap();
        assert!(matches!(host.start().await, Err(HostError::Start { .. })));
        assert_eq!(*log.lock().unwrap(), ["start:extension", "stop:extension"]);
    }
}

#[tokio::test]
async fn cleanup_failure_retains_dependency_but_does_not_leak_independent_sibling() {
    let log = Log::default();
    let mut host = Host::default();
    let mut agent = fixture("z-agent", false, true, &log);
    agent.fail_stop_once = true;
    host.register(Box::new(agent)).unwrap();
    host.register(Box::new(fixture("a-storage", true, false, &log)))
        .unwrap();
    host.register(Box::new(fixture("b-sibling", false, false, &log)))
        .unwrap();
    host.start().await.unwrap();
    assert_eq!(host.shutdown().await.len(), 1);
    assert_eq!(host.state(), HostState::Failed);
    assert_eq!(
        *log.lock().unwrap(),
        [
            "start:a-storage",
            "start:b-sibling",
            "start:z-agent",
            "stop:z-agent",
            "stop:b-sibling"
        ]
    );
    assert!(host.service::<JournalService>().is_err());
    assert!(host.shutdown().await.is_empty());
    assert_eq!(host.state(), HostState::Idle);
    assert_eq!(
        &log.lock().unwrap()[5..],
        ["stop:z-agent", "stop:a-storage"]
    );
}

#[tokio::test]
async fn cancelling_start_future_does_not_forget_partial_setup_ownership() {
    let log = Log::default();
    let mut host = Host::default();
    let mut agent = fixture("agent", false, true, &log);
    agent.pending_start = true;
    host.register(Box::new(agent)).unwrap();
    host.register(Box::new(fixture("storage", true, false, &log)))
        .unwrap();
    let mut start = Box::pin(host.start());
    assert!(matches!(
        start.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(start);
    assert_eq!(host.state(), HostState::Starting);
    assert!(host.shutdown().await.is_empty());
    assert_eq!(
        *log.lock().unwrap(),
        ["start:storage", "start:agent", "stop:agent", "stop:storage"]
    );
}

#[tokio::test]
async fn cancelling_teardown_does_not_release_dependency_early_or_forget_owner() {
    let log = Log::default();
    let mut host = Host::default();
    let mut agent = fixture("agent", false, true, &log);
    agent.pending_stop_once = true;
    host.register(Box::new(agent)).unwrap();
    host.register(Box::new(fixture("storage", true, false, &log)))
        .unwrap();
    host.start().await.unwrap();
    let mut stop = Box::pin(host.shutdown());
    assert!(matches!(
        stop.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    drop(stop);
    assert_eq!(host.state(), HostState::Stopping);
    assert!(host.shutdown().await.is_empty());
    assert_eq!(
        *log.lock().unwrap(),
        [
            "start:storage",
            "start:agent",
            "stop:agent",
            "stop:agent",
            "stop:storage"
        ]
    );
}
