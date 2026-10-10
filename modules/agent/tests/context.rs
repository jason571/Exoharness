use exoharness_agent::text_request;
use exoharness_contracts::model::Message;
use serde_json::json;
#[test]
fn context_keeps_complete_pairs_and_excludes_failed_turns() {
    let request = text_request(
        "identity",
        &json!({"turns":[
        {"prompt":"old","answer":"a","status":"succeeded"},
        {"prompt":"bad","answer":"partial","status":"failed"}]}),
        "now",
        1024,
    )
    .unwrap();
    assert_eq!(request.messages.len(), 4);
    assert!(matches!(&request.messages[3],Message::User{text} if text=="now"));
}
#[test]
fn oversized_history_is_omitted_not_sliced_and_newest_prompt_is_exact() {
    let request = text_request(
        "identity",
        &json!({"turns":[{"prompt":"x","answer":"a".repeat(40000),"status":"succeeded"}]}),
        "{\"must_remain\":true}",
        1024,
    )
    .unwrap();
    assert_eq!(request.messages.len(), 2);
    assert!(matches!(&request.messages[1],Message::User{text} if text=="{\"must_remain\":true}"));
}
