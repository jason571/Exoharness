use exoharness_storage_postgres::{ExternalSessionRequest, PostgresStore, StoreError};
use serde_json::{Value, json};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
fn fixture() -> PostgresStore {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_ext_sessions_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = PostgresStore::connect(
        &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").unwrap(),
        &namespace,
    )
    .unwrap();
    store.migrate().unwrap();
    store
}
fn profile() -> Value {
    json!({"id":"carenest","display_name":"CareNest","revision":1})
}
#[test]
fn concurrent_session_creation_is_idempotent_and_keeps_input_and_application_boundaries() {
    let store = Arc::new(fixture());
    let barrier = Arc::new(Barrier::new(4));
    let jobs = (0..4)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .create_external_session(ExternalSessionRequest {
                        owner: "user_one",
                        client_id: "app_a",
                        request_key: "same-conversation",
                        title: Some("A"),
                        profile: &profile(),
                    })
                    .unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
        })
        .collect::<Vec<_>>();
    let ids = jobs
        .into_iter()
        .map(|j| j.join().unwrap())
        .collect::<Vec<_>>();
    assert!(ids.iter().all(|s| s == &ids[0]));
    assert!(matches!(
        store.create_external_session(ExternalSessionRequest {
            owner: "user_one",
            client_id: "app_a",
            request_key: "same-conversation",
            title: Some("changed"),
            profile: &profile()
        }),
        Err(StoreError::Conflict)
    ));
    let other = store
        .create_external_session(ExternalSessionRequest {
            owner: "user_one",
            client_id: "app_b",
            request_key: "same-conversation",
            title: Some("B"),
            profile: &profile(),
        })
        .unwrap();
    assert_ne!(other["id"], ids[0]);
    assert!(
        !store
            .external_session_authorized("user_two", "app_a", "carenest", &ids[0])
            .unwrap()
    );
    assert!(
        !store
            .external_session_authorized("user_one", "app_b", "carenest", &ids[0])
            .unwrap()
    );
    assert!(
        !store
            .external_session_authorized("user_one", "app_a", "another-assistant", &ids[0])
            .unwrap()
    );
}
#[test]
fn invalid_or_archived_sessions_are_not_replaced_on_retry() {
    let store = fixture();
    assert!(
        store
            .create_external_session(ExternalSessionRequest {
                owner: "user",
                client_id: "app",
                request_key: "",
                title: None,
                profile: &profile()
            })
            .is_err()
    );
    let first = store
        .create_external_session(ExternalSessionRequest {
            owner: "user",
            client_id: "app",
            request_key: "stable",
            title: None,
            profile: &profile(),
        })
        .unwrap();
    let id = first["id"].as_str().unwrap();
    store.delete_conversation("user", id).unwrap();
    assert!(matches!(
        store.create_external_session(ExternalSessionRequest {
            owner: "user",
            client_id: "app",
            request_key: "stable",
            title: None,
            profile: &profile()
        }),
        Err(StoreError::Conflict)
    ));
    assert!(
        !store
            .external_session_authorized("user", "app", "carenest", id)
            .unwrap()
    );
}
