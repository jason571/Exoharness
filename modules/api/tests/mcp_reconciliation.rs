//! Real PostgreSQL + HTTP MCP protocol. No OCI, model requests or video generation.
#[path = "../../mcp/tests/support/mod.rs"]
#[allow(dead_code)]
mod remote;
use exoharness_api::{reconcile_mcp_page, reconcile_mcp_tasks};
use exoharness_artifacts::Artifacts;
use exoharness_configuration::Assistants;
use exoharness_contracts::{
    capability::PermissionDecision, invocation::InvocationState, model::ToolCall,
};
use exoharness_mcp::Mcp;
use exoharness_model_runtime::CancellationToken;
use exoharness_registry::Registry;
use exoharness_secrets::SecretStore;
use exoharness_skills::Skills;
use exoharness_storage_postgres::{
    CreateTurn, ExecutionSnapshot, InvocationAdmission, InvocationIntent, ModelSnapshot,
    PostgresStore, RegistryAccess, RunState,
};
use exoharness_tools::ToolRuntime;
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;
const CAP: &str = "mcp_server:global:_:fixture";

struct Fixture {
    _root: tempfile::TempDir,
    tools: Arc<ToolRuntime>,
    assistants: Arc<Assistants>,
    server: remote::Server,
    url: String,
    namespace: String,
}
impl Fixture {
    async fn new(config: remote::StateData) -> Self {
        let server = remote::server(config).await;
        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL")
            .expect("isolated PostgreSQL required");
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let namespace = format!(
            "pro_reconcile_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let (s, r) = (url.clone(), namespace.clone());
        let (store, registry) = tokio::task::spawn_blocking(move || {
            let store = PostgresStore::connect(&s, &r).unwrap();
            store.migrate().unwrap();
            let registry = PostgresStore::connect(&s, &format!("{r}_registry")).unwrap();
            registry.migrate_registry().unwrap();
            (store, Arc::new(Registry(registry)))
        })
        .await
        .unwrap();
        let secrets = Arc::new(SecretStore::open(root.path().join("secrets")).unwrap());
        let mcp = Arc::new(Mcp::new(registry.clone(), secrets));
        let assistants = Arc::new(Assistants::initialize(&root.path().join("assistants")).unwrap());
        assistants.create("test",json!({"display_name":"Test","avatar_text":"T","description":"fixture","system_prompt":"test",
            "skill_ids":[],"access":{"skills":{"mode":"none"},"mcp":{"mode":"all"},"tools":{"mode":"none"},"models":{"mode":"none"},"published":true}}),true,true).unwrap();
        let skills = Arc::new(Skills::open(root.path().join("skills"), registry.clone()).unwrap());
        let artifacts = Arc::new(
            Artifacts::open(
                root.path().join("artifacts"),
                store.clone(),
                "http://127.0.0.1:9790",
            )
            .unwrap(),
        );
        let tools = Arc::new(ToolRuntime {
            execution: store,
            registry,
            skills,
            mcp,
            sandbox: None,
            artifacts,
            model_runtime: None,
        });
        let (t, endpoint) = (tools.clone(), server.url.clone());
        tokio::task::spawn_blocking(move || {
            let record=t.mcp.import(&RegistryAccess::Administrator,&serde_json::from_value(json!({"content":json!({"mcpServers":{"fixture":{"type":"streamableHttp","url":endpoint}}}).to_string()})).unwrap(),"admin").unwrap().remove(0);
            assert_eq!(record.id,CAP);
            for permission in &record.manifest.permissions {
                t.registry.0.decide_capability_permission(&RegistryAccess::Administrator,&record.id,record.revision,permission,PermissionDecision::Allow,"admin").unwrap();
            }
            t.registry.0.set_capability_enabled(&RegistryAccess::Administrator,&record.id,record.revision,true,"admin").unwrap();
        }).await.unwrap();
        let validation = tools
            .mcp
            .validate(RegistryAccess::Administrator, CAP.into(), "admin".into())
            .await
            .unwrap();
        assert_eq!(validation["status"], "passed", "{validation}");
        Self {
            _root: root,
            tools,
            assistants,
            server,
            url,
            namespace,
        }
    }
    async fn prepare(&self, owner: &str, id: &str) -> (String, String, String) {
        let (tools, profile, owner, id) = (
            self.tools.clone(),
            self.assistants.detail("test").unwrap(),
            owner.to_owned(),
            id.to_owned(),
        );
        tokio::task::spawn_blocking(move || {
            let mut catalog = tools.catalog(&owner, profile.clone()).unwrap();
            let name = exoharness_tools::public_name(CAP, "echo");
            catalog.activate(std::slice::from_ref(&name)).unwrap();
            let snapshot = ExecutionSnapshot {
                version: 1,
                assistant_id: "test".into(),
                assistant_revision: profile["active_revision"].as_u64().unwrap(),
                model: ModelSnapshot {
                    config_id: "fixture".into(),
                    revision: 1,
                    adapter: "fixture".into(),
                    settings: json!({}),
                },
                capabilities: catalog.snapshots(),
                max_consecutive_identical_calls: 3,
                requires_execution: true,
            };
            let session = format!("session-{id}");
            let run = format!("run-{id}");
            let invocation = format!("invocation-{id}");
            let store = &tools.execution;
            store.create_session(&owner, &session).unwrap();
            let r = store
                .create_turn(CreateTurn {
                    owner: &owner,
                    session_id: &session,
                    expected_session_version: store.session_version(&owner, &session).unwrap(),
                    turn_id: &format!("turn-{id}"),
                    run_id: &run,
                    input: "generate once",
                    snapshot: &snapshot,
                })
                .unwrap();
            store
                .transition_run(&owner, &session, &run, r.version, RunState::Running, None)
                .unwrap();
            store
                .prepare_invocation(
                    &owner,
                    &session,
                    &run,
                    &InvocationIntent {
                        id: invocation.clone(),
                        logical_key: invocation.clone(),
                        capability_id: CAP.into(),
                        capability_revision: catalog.binding(&name).unwrap().revision,
                        tool: name,
                        arguments: json!({"text":"start once"}),
                    },
                )
                .unwrap();
            (session, run, invocation)
        })
        .await
        .unwrap()
    }
    async fn cancelled(&self) -> (String, String, String) {
        let call = self.prepare("user", "first").await;
        let (tools, profile) = (self.tools.clone(), self.assistants.detail("test").unwrap());
        let mut catalog =
            tokio::task::spawn_blocking(move || tools.catalog("user", profile).unwrap())
                .await
                .unwrap();
        let name = exoharness_tools::public_name(CAP, "echo");
        catalog.activate(std::slice::from_ref(&name)).unwrap();
        let (tools, saved) = (self.tools.clone(), call.clone());
        tokio::task::spawn_blocking(move || {
            tools
                .execution
                .claim_mcp_invocation("user", &saved.0, &saved.1, &saved.2, 1)
                .unwrap()
        })
        .await
        .unwrap();
        let token = CancellationToken::default();
        let (tools, saved, cancel) = (self.tools.clone(), call.clone(), token.clone());
        let (progress, _rx) = watch::channel(None);
        let worker = tokio::spawn(async move {
            tools
                .dispatch(
                    &mut catalog,
                    "user",
                    &saved.0,
                    &saved.1,
                    &saved.2,
                    &ToolCall {
                        id: "fixture-call".into(),
                        name,
                        arguments: json!({"text":"start once"}),
                    },
                    cancel,
                    progress,
                )
                .await
        });
        let mut persisted = false;
        for _ in 0..100 {
            let (store, saved) = (self.tools.execution.clone(), call.clone());
            if tokio::task::spawn_blocking(move || store.events("user",&saved.0,&saved.1,0,100).unwrap()
                .iter().any(|e|matches!(&e.event,exoharness_storage_postgres::ExecutionEvent::Chat{payload} if payload["type"]=="mcp_background_task"))).await.unwrap() {persisted=true;break;}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(persisted);
        let (store, saved) = (self.tools.execution.clone(), call.clone());
        tokio::task::spawn_blocking(move || store.chat_cancel("user", &saved.1).unwrap())
            .await
            .unwrap();
        token.cancel();
        assert!(matches!(
            worker.await.unwrap(),
            Err(exoharness_tools::ToolError::Unknown)
        ));
        let (store, saved) = (self.tools.execution.clone(), call.clone());
        tokio::task::spawn_blocking(move || {
            store
                .resolve_invocation(
                    "user",
                    &saved.0,
                    &saved.1,
                    &saved.2,
                    InvocationState::OutcomeUnknown,
                    None,
                )
                .unwrap()
        })
        .await
        .unwrap();
        call
    }
    async fn poll(&self) {
        assert_eq!(
            reconcile_mcp_page(self.tools.clone(), self.assistants.clone(), String::new())
                .await
                .unwrap(),
            ""
        );
    }
    async fn receipt(
        &self,
        call: &(String, String, String),
    ) -> exoharness_storage_postgres::InvocationRecord {
        let (store, call) = (self.tools.execution.clone(), call.clone());
        tokio::task::spawn_blocking(move || {
            store.invocation("user", &call.0, &call.1, &call.2).unwrap()
        })
        .await
        .unwrap()
    }
    fn count(&self, name: &str) -> usize {
        self.server
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == name)
            .count()
    }
    async fn close(self) {
        let (url, namespace) = (self.url.clone(), self.namespace.clone());
        tokio::task::spawn_blocking(move || {
            // Test namespaces are private and drop with their dependent registry.
            let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
            client
                .batch_execute(&format!(
                    "DROP SCHEMA {namespace} CASCADE; DROP SCHEMA {namespace}_registry CASCADE"
                ))
                .unwrap();
        })
        .await
        .unwrap();
    }
}
fn delayed() -> remote::StateData {
    remote::StateData {
        background: true,
        background_cancel_pending: true,
        background_delayed_cancel: true,
        background_never_complete: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn delayed_cancel_releases_shared_capacity_only_after_verified_terminal_receipt() {
    let f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    let second = f.prepare("other-user", "second").await;
    f.poll().await;
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    let (store, call) = (f.tools.execution.clone(), second.clone());
    assert!(matches!(
        tokio::task::spawn_blocking(move || store.claim_mcp_invocation(
            "other-user",
            &call.0,
            &call.1,
            &call.2,
            1
        ))
        .await
        .unwrap(),
        Err(exoharness_storage_postgres::StoreError::OutcomeUnknown)
    ));
    f.server
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    for _ in 0..5 {
        f.poll().await;
    }
    let receipt = f.receipt(&first).await;
    assert_eq!(receipt.state, InvocationState::FailedAfterExecution);
    assert_eq!(
        receipt.result.unwrap()["structuredContent"]["status"],
        "cancelled"
    );
    let (store, call) = (f.tools.execution.clone(), second.clone());
    assert_eq!(
        tokio::task::spawn_blocking(move || store
            .claim_mcp_invocation("other-user", &call.0, &call.1, &call.2, 1)
            .unwrap())
        .await
        .unwrap(),
        InvocationAdmission::Claimed
    );
    let (store, call) = (f.tools.execution.clone(), first.clone());
    assert_eq!(
        tokio::task::spawn_blocking(move || store.run("user", &call.0, &call.1).unwrap().state)
            .await
            .unwrap(),
        RunState::Cancelled
    );
    assert_eq!(f.count("echo"), 1);
    assert_eq!(f.count("cancel"), 1);
    for request in f
        .server
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["params"]["name"] == "poll")
    {
        assert_eq!(request["params"]["arguments"]["job_id"], "fixture-job");
    }
    f.close().await;
}

#[tokio::test]
async fn wrong_job_receipt_never_releases_capacity() {
    let f = Fixture::new(remote::StateData {
        background_wrong_id: true,
        ..delayed()
    })
    .await;
    let first = f.cancelled().await;
    f.server
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    for _ in 0..5 {
        f.poll().await;
    }
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    assert_eq!(f.count("echo"), 1);
    assert_eq!(f.count("cancel"), 1);
    f.close().await;
}

#[tokio::test]
async fn poll_outage_preserves_unknown_then_recovers_without_resubmission() {
    let f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    f.server
        .state
        .http_faults
        .lock()
        .unwrap()
        .insert("tools/call:poll".into(), [503, 503].into());
    f.poll().await;
    f.poll().await;
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    f.server
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    for _ in 0..5 {
        f.poll().await;
    }
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::FailedAfterExecution
    );
    assert_eq!(f.count("echo"), 1);
    assert_eq!(f.count("cancel"), 1);
    f.close().await;
}

#[tokio::test]
async fn revoked_capability_blocks_recovery_poll() {
    let f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    let registry = f.tools.registry.clone();
    tokio::task::spawn_blocking(move || {
        let record = registry
            .0
            .capability(&RegistryAccess::Administrator, CAP)
            .unwrap();
        registry
            .0
            .set_capability_enabled(
                &RegistryAccess::Administrator,
                CAP,
                record.revision,
                false,
                "admin",
            )
            .unwrap();
    })
    .await
    .unwrap();
    f.poll().await;
    assert_eq!(f.count("poll"), 0);
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    f.close().await;
}

#[tokio::test]
async fn restart_and_archiving_retain_owned_task_and_preserve_cancelled_run() {
    let mut f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    let (url, namespace, session) = (f.url.clone(), f.namespace.clone(), first.0.clone());
    let reopened = tokio::task::spawn_blocking(move || {
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        client
            .execute(
                &format!("UPDATE {namespace}.execution_sessions SET archived=1 WHERE id=$1"),
                &[&session],
            )
            .unwrap();
        PostgresStore::connect(&url, &namespace).unwrap()
    })
    .await
    .unwrap();
    let old = &f.tools;
    f.tools = Arc::new(ToolRuntime {
        execution: reopened,
        registry: old.registry.clone(),
        skills: old.skills.clone(),
        mcp: old.mcp.clone(),
        sandbox: None,
        artifacts: old.artifacts.clone(),
        model_runtime: None,
    });
    f.server
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    for _ in 0..5 {
        f.poll().await;
    }
    let (url, namespace, run) = (f.url.clone(), f.namespace.clone(), first.1.clone());
    tokio::task::spawn_blocking(move || {
        let mut db = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        assert_eq!(
            db.query_one(
                &format!("SELECT status FROM {namespace}.execution_runs WHERE id=$1"),
                &[&run]
            )
            .unwrap()
            .get::<_, String>(0),
            "cancelled"
        );
        assert_eq!(
            db.query_one(
                &format!(
                    "SELECT status FROM {namespace}.execution_invocations WHERE latest_run_id=$1"
                ),
                &[&run]
            )
            .unwrap()
            .get::<_, String>(0),
            "completed"
        );
    })
    .await
    .unwrap();
    assert_eq!(f.count("echo"), 1);
    assert_eq!(f.count("cancel"), 1);
    f.close().await;
}

#[tokio::test]
async fn late_success_is_saved_without_resuming_or_submitting_work() {
    let f = Fixture::new(remote::StateData {
        background_delayed_cancel: false,
        ..delayed()
    })
    .await;
    let first = f.cancelled().await;
    f.server
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    for _ in 0..5 {
        f.poll().await;
    }
    assert_eq!(f.receipt(&first).await.state, InvocationState::Succeeded);
    assert_eq!(f.count("echo"), 1);
    assert_eq!(f.count("cancel"), 1);
    f.close().await;
}

#[tokio::test]
async fn owned_worker_stops_promptly_during_a_slow_poll() {
    let f = Fixture::new(remote::StateData {
        background_poll_delay_ms: 30000,
        ..delayed()
    })
    .await;
    let first = f.cancelled().await;
    let (stop, rx) = watch::channel(false);
    let worker = tokio::spawn(reconcile_mcp_tasks(
        f.tools.clone(),
        f.assistants.clone(),
        rx,
    ));
    for _ in 0..100 {
        if f.count("poll") > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    f.close().await;
}

#[tokio::test]
async fn changed_assistant_revision_blocks_recovery_without_network_calls() {
    let f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    let profile = f.assistants.detail("test").unwrap();
    let fields = [
        "display_name",
        "avatar_text",
        "avatar",
        "description",
        "system_prompt",
        "skill_ids",
        "access",
        "default_llm",
    ];
    let input = serde_json::Value::Object(
        fields
            .into_iter()
            .filter_map(|key| profile.get(key).map(|value| (key.into(), value.clone())))
            .collect(),
    );
    f.assistants.update("test", 1, input).unwrap();
    f.poll().await;
    assert_eq!(f.count("poll"), 0);
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    f.close().await;
}

#[tokio::test]
async fn changed_status_schema_at_same_revision_cannot_broaden_saved_poll() {
    let f = Fixture::new(delayed()).await;
    let first = f.cancelled().await;
    let registry = f.tools.registry.clone();
    tokio::task::spawn_blocking(move || {
        let access = RegistryAccess::Administrator;
        let record = registry.0.capability(&access, CAP).unwrap();
        let mut tools = registry.0.discovered_tools(&access, CAP).unwrap();
        tools
            .iter_mut()
            .find(|tool| tool["name"] == "poll")
            .unwrap()["inputSchema"]["additionalProperties"] = json!(false);
        registry
            .0
            .observe_capability(
                &access,
                CAP,
                record.revision,
                Some(&tools),
                &json!({}),
                "admin",
            )
            .unwrap();
    })
    .await
    .unwrap();
    f.poll().await;
    assert_eq!(f.count("poll"), 0);
    assert_eq!(
        f.receipt(&first).await.state,
        InvocationState::OutcomeUnknown
    );
    f.close().await;
}
