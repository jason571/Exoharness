use exoharness_storage_postgres::{
    CreateTurn, ExecutionSnapshot, ModelSnapshot, PostgresStore, RunState,
};
use serde_json::{Value, json};

fn turn(store: &PostgresStore, session: &str, suffix: &str, input: &str) {
    store
        .create_turn(CreateTurn {
            owner: "owner",
            session_id: session,
            expected_session_version: store.session_version("owner", session).unwrap(),
            turn_id: &format!("turn-{suffix}"),
            run_id: &format!("run-{suffix}"),
            input,
            snapshot: &ExecutionSnapshot {
                version: 1,
                assistant_id: "a".into(),
                assistant_revision: 1,
                model: ModelSnapshot {
                    config_id: "m".into(),
                    revision: 1,
                    adapter: "mock".into(),
                    settings: json!({}),
                },
                capabilities: vec![],
                max_consecutive_identical_calls: 3,
                requires_execution: false,
            },
        })
        .unwrap();
}
#[test]
fn first_message_sets_a_bounded_unicode_title_without_extra_model_calls_or_overwriting_manual_names()
 {
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated PostgreSQL required");
    let schema = format!("pro_titles_{}", std::process::id());
    let store = PostgresStore::connect(&url, &schema).unwrap();
    store.migrate().unwrap();
    store
        .new_conversation_with_title("owner", "auto", "新对话", &Value::Null, true)
        .unwrap();
    let prompt = format!("  给小朋友\n讲解太阳系 {}", "中文😀".repeat(40));
    turn(&store, "auto", "first", &prompt);
    let title = store.conversations("owner", "").unwrap()[0]["title"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(title.starts_with("给小朋友 讲解太阳系"));
    assert_eq!(title.chars().count(), 61);
    assert!(title.ends_with('…'));
    assert_eq!(store.conversations("owner", "太阳系").unwrap().len(), 1);
    let run = store.owned_run("owner", "run-first").unwrap();
    store
        .transition_run(
            "owner",
            "auto",
            "run-first",
            run.version,
            RunState::Cancelled,
            None,
        )
        .unwrap();
    turn(&store, "auto", "second", "不要改写首条标题");
    assert_eq!(
        store.conversations("owner", "太阳系").unwrap()[0]["title"],
        title
    );
    store
        .new_conversation_with_title("owner", "manual", "新对话", &Value::Null, true)
        .unwrap();
    store
        .update_conversation("owner", "manual", &json!({"title":"新对话"}))
        .unwrap();
    turn(&store, "manual", "manual", "已手动命名，不应覆盖");
    assert_eq!(
        store
            .conversations("owner", "")
            .unwrap()
            .iter()
            .find(|s| s["id"] == "manual")
            .unwrap()["title"],
        "新对话"
    );
    store
        .new_conversation("owner", "named", "我的工作记录", &Value::Null)
        .unwrap();
    turn(&store, "named", "named", "保持创建时的自定义名称");
    assert_eq!(store.conversations("owner", "我的工作").unwrap().len(), 1);
    assert!(store.conversations("other", "").unwrap().is_empty());
    let reopened = PostgresStore::connect(&url, &schema).unwrap();
    reopened.migrate().unwrap();
    assert_eq!(
        reopened.conversations("owner", "太阳系").unwrap()[0]["title"],
        title
    );
    assert_eq!(
        reopened
            .conversations("owner", "")
            .unwrap()
            .iter()
            .find(|s| s["id"] == "manual")
            .unwrap()["title"],
        "新对话"
    );
}
#[test]
fn existing_placeholder_titles_are_backfilled_once_and_custom_titles_remain_intact() {
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated PostgreSQL required");
    let schema = format!("pro_title_legacy_{}", std::process::id());
    let store = PostgresStore::connect(&url, &schema).unwrap();
    store.migrate().unwrap();
    store
        .new_conversation("owner", "old", "旧标题", &Value::Null)
        .unwrap();
    turn(&store, "old", "legacy", "这是之前会话的实际主题");
    let mut conn = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    conn.batch_execute(&format!("SET search_path TO {schema}; UPDATE execution_sessions SET title='新对话',title_source='legacy' WHERE id='old'")).unwrap();
    store.migrate().unwrap();
    assert_eq!(
        store.conversations("owner", "").unwrap()[0]["title"],
        "这是之前会话的实际主题"
    );
    store
        .update_conversation("owner", "old", &json!({"title":"手动新名称"}))
        .unwrap();
    store.migrate().unwrap();
    assert_eq!(
        store.conversations("owner", "").unwrap()[0]["title"],
        "手动新名称"
    );
}
