use exoharness_agent::text_request;
use exoharness_contracts::model::Message;
use serde_json::json;
#[test]
fn context_keeps_failed_request_without_claiming_partial_answer_is_successful() {
    let request = text_request(
        "identity",
        &json!({"turns":[
        {"prompt":"old","answer":"a","status":"succeeded"},
        {"prompt":"bad","answer":"partial","status":"failed"}]}),
        "now",
        1024,
    )
    .unwrap();
    assert_eq!(request.messages.len(), 6);
    assert!(matches!(&request.messages[3],Message::User{text} if text=="bad"));
    assert!(
        matches!(&request.messages[4],Message::Assistant{text,..} if text.contains("unfinished") && !text.contains("partial"))
    );
    assert!(matches!(&request.messages[5],Message::User{text} if text=="now"));
}

#[test]
fn latest_eight_turns_keep_failed_question_and_exclude_cancelled_and_running_work() {
    let mut turns: Vec<_> = (0..1000)
        .map(|n| json!({"prompt":format!("old-{n}"),"answer":"ok","status":"succeeded"}))
        .collect();
    turns
        .push(json!({"prompt":"original multiplication question","answer":null,"status":"failed"}));
    turns.push(json!({"prompt":"cancelled task","answer":"unfinished","status":"cancelled"}));
    turns.push(json!({"prompt":"running task","answer":null,"status":"running"}));
    let request = text_request("system", &json!({"turns":turns}), "继续生成", 1024).unwrap();
    let text = serde_json::to_string(&request).unwrap();
    assert!(text.contains("original multiplication question"));
    assert!(!text.contains("old-0"));
    assert!(!text.contains("cancelled task"));
    assert!(!text.contains("running task"));
    assert!(request.messages.len() <= 18);
}
#[test]
fn user_visible_partial_answer_remains_context_without_claiming_task_completion() {
    let request=text_request("system",&json!({"turns":[{"prompt":"multiplication question","answer":"13×24=312; animation unavailable","status":"failed","response":{"partial_result":true}}]}),"Why 13?",1024).unwrap();
    let text = serde_json::to_string(&request).unwrap();
    assert!(text.contains("13×24=312"));
    assert!(text.contains("unfinished"));
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
