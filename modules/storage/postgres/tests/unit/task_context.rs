use super::*;

#[test]
fn failed_task_context_preserves_receipts_and_rechecks_owner_revision_and_archive() {
    let (store, _, _) = database();
    let run = running(&store);
    let prepared = store
        .prepare_invocation("owner", "session", "run", &intent("saved-audio", 1))
        .unwrap();
    store
        .claim_invocation("owner", "session", "run", &prepared.intent.id)
        .unwrap();
    store
        .resolve_invocation(
            "owner",
            "session",
            "run",
            &prepared.intent.id,
            I::Succeeded,
            Some(json!({"status":"completed","audio":{"url":"https://media.example/saved.mp3"}})),
        )
        .unwrap();
    store
        .finish_chat(
            "owner",
            "session",
            "run",
            json!({"answer":"","usage":{"input_tokens":10,"output_tokens":2}}),
            Some("trace-only failure"),
        )
        .unwrap();
    let context = store
        .chat_task_context("owner", "session", "run", &run.snapshot.capabilities)
        .unwrap();
    assert!(context.to_string().contains("saved.mp3"));
    assert_eq!(context["original_prompt"], "perform task");
    assert_eq!(context["steps"][0]["outcome"], "success");
    assert!(
        store
            .chat_task_context("other", "session", "run", &run.snapshot.capabilities)
            .is_err()
    );
    assert!(
        store
            .chat_task_context("owner", "other-session", "run", &run.snapshot.capabilities)
            .is_err()
    );
    assert_eq!(
        store
            .chat_task_context("owner", "session", "run", &[])
            .unwrap()["steps"],
        json!([])
    );
    let mut changed = run.snapshot.capabilities.clone();
    changed[0].revision += 1;
    assert_eq!(
        store
            .chat_task_context("owner", "session", "run", &changed)
            .unwrap()["steps"],
        json!([])
    );
    store.delete_conversation("owner", "session").unwrap();
    assert!(
        store
            .chat_task_context("owner", "session", "run", &run.snapshot.capabilities)
            .is_err()
    );
}
