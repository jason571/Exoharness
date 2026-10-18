use exoharness_storage_postgres::{
    ExternalConversationQuery, ExternalSessionRequest, PostgresStore, StoreError,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
fn fixture() -> (PostgresStore, postgres::Client) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_ext_inspect_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").unwrap();
    let store = PostgresStore::connect(&url, &namespace).unwrap();
    store.migrate().unwrap();
    store.migrate().unwrap();
    let mut db = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    db.batch_execute(&format!("SET search_path={namespace},public"))
        .unwrap();
    (store, db)
}
fn create(s: &PostgresStore, client: &str, user: &str, key: &str, device: Option<&str>) -> Value {
    s.create_external_session_with_device(
        ExternalSessionRequest {
            owner: user,
            client_id: client,
            request_key: key,
            title: Some(key),
            profile: &json!({"id":"role"}),
        },
        device,
    )
    .unwrap()
}
fn query(page: usize) -> ExternalConversationQuery {
    ExternalConversationQuery {
        client_id: Some("app".into()),
        query: String::new(),
        matching_users: vec![],
        user_ids: None,
        device: String::new(),
        page,
        limit: 10,
    }
}
#[test]
fn pages_search_counts_content_and_scope_exclude_ordinary_or_archived_sessions() {
    let (s, mut db) = fixture();
    let mut ids = vec![];
    for i in 0..23 {
        ids.push(
            create(
                &s,
                "app",
                "user",
                &format!("conversation-{i}"),
                Some("watch_a"),
            )["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    create(&s, "other", "user", "another-app", Some("watch_b"));
    s.new_conversation("user", "ordinary", "ordinary", &json!({"id":"role"}))
        .unwrap();
    let archived = create(&s, "app", "user", "archived", Some("watch_a"));
    s.delete_conversation("user", archived["id"].as_str().unwrap())
        .unwrap();
    for n in 1..=12_i64 {
        let id = format!("turn-{n}");
        let prompt = format!("private prompt {n}");
        db.execute("INSERT INTO execution_turns(id,session_id,ordinal,prompt,history_content,attachments_json,capability_ids_json,status,answer,response_json,created_at,updated_at) VALUES($1,$2,$3,$4,$4,'[]','[]','succeeded','answer%literal','{}',0,0)",&[&id,&ids[0],&n,&prompt]).unwrap();
    }
    let mut seen = std::collections::BTreeSet::new();
    for page in 1..=3 {
        let result = s.management_external_conversations(query(page)).unwrap();
        assert_eq!(result["total"], 23);
        assert_eq!(result["has_more"], page < 3);
        let rows = result["sessions"].as_array().unwrap();
        assert_eq!(rows.len(), if page == 3 { 3 } else { 10 });
        for row in rows {
            assert!(seen.insert(row["id"].as_str().unwrap().to_owned()));
        }
    }
    assert_eq!(seen.len(), 23);
    let mut q = query(1);
    q.query = "%literal".into();
    let result = s.management_external_conversations(q).unwrap();
    assert_eq!(result["total"], 1);
    assert_eq!(result["sessions"][0]["turn_count"], 12);
    let detail = s
        .management_external_conversation("app", &ids[0], None, 10)
        .unwrap();
    assert_eq!(detail["turns"].as_array().unwrap().len(), 10);
    assert_eq!(detail["turns"][0]["ordinal"], 3);
    assert_eq!(detail["external"]["turn_count"], 12);
    let older = s
        .management_external_conversation("app", &ids[0], Some(3), 10)
        .unwrap();
    assert_eq!(older["turns"].as_array().unwrap().len(), 2);
    assert_eq!(older["history_page"]["has_more"], false);
    assert!(matches!(
        s.management_external_conversation("other", &ids[0], None, 10),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        s.management_external_conversation("app", "ordinary", None, 10),
        Err(StoreError::NotFound)
    ));
    let mut q = query(1);
    q.limit = 11;
    assert!(s.management_external_conversations(q).is_err());
    let mut q = query(1);
    q.device = "watch_b".into();
    assert_eq!(s.management_external_conversations(q).unwrap()["total"], 0);
}
#[test]
fn legacy_metadata_can_be_completed_once_without_replacing_a_session_or_crossing_owners() {
    let (s, _) = fixture();
    let first = create(&s, "app", "user", "legacy", None);
    let id = first["id"].as_str().unwrap();
    assert_eq!(s.external_session_device("user", "app", id).unwrap(), None);
    assert!(matches!(
        s.set_external_session_device("another", "app", "role", id, "watch"),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        s.set_external_session_device("user", "other", "role", id, "watch"),
        Err(StoreError::NotFound)
    ));
    s.set_external_session_device("user", "app", "role", id, "watch")
        .unwrap();
    assert_eq!(
        s.external_session_device("user", "app", id)
            .unwrap()
            .as_deref(),
        Some("watch")
    );
    assert_eq!(create(&s, "app", "user", "legacy", Some("watch"))["id"], id);
    assert_eq!(create(&s, "app", "user", "legacy", None)["id"], id);
    assert!(matches!(
        s.set_external_session_device("user", "app", "role", id, "different"),
        Err(StoreError::Conflict)
    ));
    for invalid in ["".to_owned(), " newline\n".to_owned(), "x".repeat(129)] {
        assert!(
            s.set_external_session_device("user", "app", "role", id, &invalid)
                .is_err()
        );
    }
}
