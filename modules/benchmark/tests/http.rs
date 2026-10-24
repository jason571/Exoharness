mod support;
use exoharness_benchmark::{BenchmarkHarness, Harness, Suite, campaign};
use serde_json::json;
fn suite() -> Suite {
    Suite::parse(json!({"version":1,"id":"http_suite","tasks":[{"id":"train_task","group":"math","split":"train","turns":["Training amount"],"checks":[{"kind":"json_equals","fields":{"amount":42}}]},{"id":"test_task","group":"math","split":"test","turns":["Testing amount"],"checks":[{"kind":"json_equals","fields":{"amount":42}}]}]})).unwrap()
}
#[tokio::test]
async fn real_http_submission_trace_and_resume_do_not_repeat() {
    let f = support::Fixture::start(100, json!({})).await;
    let temp = tempfile::tempdir().unwrap();
    let auth = temp.path().join("auth.json");
    f.auth(&auth);
    let port = Harness::new(&auth, 1000, 30).unwrap();
    let fp = port.fingerprint().await.unwrap();
    let directory = temp.path().join("case");
    let first = port
        .execute(&directory, &["amount".into()], &fp, &[])
        .await
        .unwrap();
    let second = port
        .execute(&directory, &["amount".into()], &fp, &[])
        .await
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first["tokens"], 100);
    assert_eq!(f.state.lock().unwrap().turns, 1);
}
#[tokio::test]
async fn empty_answer_falls_back_to_response_text_and_saved_answer_like_python() {
    for (response, saved) in [
        (json!({"answer":"","text":"fallback"}), None),
        (json!({"answer":"","text":""}), Some("fallback".to_string())),
        (
            json!({"answer":null,"text":""}),
            Some("fallback".to_string()),
        ),
    ] {
        let f = support::Fixture::start(100, json!({})).await;
        {
            let mut s = f.state.lock().unwrap();
            s.response_override = Some(response);
            s.saved_answer = saved;
        }
        let temp = tempfile::tempdir().unwrap();
        let auth = temp.path().join("auth.json");
        f.auth(&auth);
        let port = Harness::new(&auth, 1000, 30).unwrap();
        let fp = port.fingerprint().await.unwrap();
        let result = port
            .execute(&temp.path().join("case"), &["answer".into()], &fp, &[])
            .await
            .unwrap();
        assert_eq!(result["text"], "fallback");
        assert_eq!(result["tokens"], 100);
    }
}
#[tokio::test]
async fn answer_precedence_preserves_nonempty_primary_text_and_empty_results() {
    for (response, saved, expected) in [
        (
            json!({"answer":"主答案","text":"secondary"}),
            Some("saved"),
            "主答案",
        ),
        (
            json!({"answer":" \n","text":"secondary"}),
            Some("saved"),
            " \n",
        ),
        (json!({"text":"文本回退"}), Some("saved"), "文本回退"),
        (
            json!({"answer":null,"text":null}),
            Some("已保存答案"),
            "已保存答案",
        ),
        (json!({}), Some("saved"), "saved"),
        (json!({"answer":"","text":""}), Some(""), ""),
        (json!({}), None, ""),
    ] {
        let f = support::Fixture::start(100, json!({})).await;
        {
            let mut state = f.state.lock().unwrap();
            state.response_override = Some(response);
            state.saved_answer = saved.map(str::to_string);
        }
        let temp = tempfile::tempdir().unwrap();
        let auth = temp.path().join("auth.json");
        f.auth(&auth);
        let port = Harness::new(&auth, 1000, 30).unwrap();
        let fingerprint = port.fingerprint().await.unwrap();
        let directory = temp.path().join("case");
        let outcome = port
            .execute(&directory, &["answer".into()], &fingerprint, &[])
            .await
            .unwrap();
        assert_eq!(outcome["text"], expected);
        assert_eq!(outcome["tokens"], 100);
        assert_eq!(
            port.execute(&directory, &["answer".into()], &fingerprint, &[])
                .await
                .unwrap(),
            outcome
        );
        assert_eq!(f.state.lock().unwrap().turns, 1);
    }
}
#[tokio::test]
async fn empty_artifact_id_falls_back_to_download_url_and_deduplicates_like_python() {
    for id in [
        json!(null),
        json!(""),
        json!(false),
        json!(0),
        json!([]),
        json!({}),
    ] {
        let f = support::Fixture::start(100, json!({})).await;
        {
            let mut s = f.state.lock().unwrap();
            s.download_artifact = true;
            s.artifact_id = id;
        }
        let temp = tempfile::tempdir().unwrap();
        let auth = temp.path().join("auth.json");
        f.auth(&auth);
        let port = Harness::new(&auth, 1000, 30).unwrap();
        let fp = port.fingerprint().await.unwrap();
        let result = port
            .execute(&temp.path().join("case"), &["page".into()], &fp, &[])
            .await
            .unwrap();
        assert_eq!(result["artifacts"].as_array().unwrap().len(), 1);
        assert_eq!(result["artifact_errors"], json!([]));
        assert_eq!(result["tokens"], 100);
        assert!(!f.state.lock().unwrap().authenticated_download);
    }
}
#[tokio::test]
async fn empty_artifact_id_fallback_still_rejects_foreign_urls_and_retains_usage() {
    let f = support::Fixture::start(100, json!({})).await;
    f.state.lock().unwrap().response_override = Some(json!({
        "answer":"done",
        "artifacts":[{"artifact_id":"","filename":"page.html","download_url":"https://example.invalid/foreign.html"}]
    }));
    let temp = tempfile::tempdir().unwrap();
    let auth = temp.path().join("auth.json");
    f.auth(&auth);
    let port = Harness::new(&auth, 1000, 30).unwrap();
    let fingerprint = port.fingerprint().await.unwrap();
    let outcome = port
        .execute(
            &temp.path().join("case"),
            &["page".into()],
            &fingerprint,
            &[],
        )
        .await
        .unwrap();
    assert!(outcome["artifacts"].as_array().unwrap().is_empty());
    assert_eq!(outcome["artifact_errors"].as_array().unwrap().len(), 1);
    assert_eq!(outcome["tokens"], 100);
    assert!(!f.state.lock().unwrap().authenticated_download);
}
#[tokio::test]
async fn uncertain_session_and_turn_posts_are_never_replayed() {
    for session in [true, false] {
        let f = support::Fixture::start(100, json!({})).await;
        {
            let mut s = f.state.lock().unwrap();
            s.lost_session = session;
            s.lost_turn = !session;
        }
        let temp = tempfile::tempdir().unwrap();
        let auth = temp.path().join("auth.json");
        f.auth(&auth);
        let port = Harness::new(&auth, 1000, 30).unwrap();
        let fp = port.fingerprint().await.unwrap();
        let directory = temp.path().join("case");
        assert!(
            port.execute(&directory, &["amount".into()], &fp, &[])
                .await
                .is_err()
        );
        assert!(
            port.execute(&directory, &["amount".into()], &fp, &[])
                .await
                .is_err()
        );
        let s = f.state.lock().unwrap();
        assert_eq!(s.sessions, 1);
        assert_eq!(s.turns, u64::from(!session));
    }
}
#[tokio::test]
async fn redirects_do_not_forward_credentials_or_lose_actual_tokens() {
    let f = support::Fixture::start(100, json!({})).await;
    f.state.lock().unwrap().artifact_redirect = true;
    let temp = tempfile::tempdir().unwrap();
    let auth = temp.path().join("auth.json");
    f.auth(&auth);
    let port = Harness::new(&auth, 1000, 30).unwrap();
    let fp = port.fingerprint().await.unwrap();
    let result = port
        .execute(&temp.path().join("case"), &["page".into()], &fp, &[])
        .await
        .unwrap();
    assert_eq!(result["tokens"], 100);
    assert_eq!(result["artifact_errors"].as_array().unwrap().len(), 1);
    assert!(!f.state.lock().unwrap().authenticated_download);
}
#[tokio::test]
async fn paired_native_campaign_preserves_all_attempts_and_model_identity() {
    let a = support::Fixture::start(100, json!({})).await;
    let b = support::Fixture::start(50, json!({})).await;
    let temp = tempfile::tempdir().unwrap();
    let ap = temp.path().join("a.json");
    let bp = temp.path().join("b.json");
    a.auth(&ap);
    b.auth(&bp);
    let base = Harness::new(&ap, 1000, 30).unwrap();
    let candidate = Harness::new(&bp, 1000, 30).unwrap();
    let options = campaign::Options {
        repeats: 3,
        max_tokens: 10000,
        run_reservation: 1000,
        input_root: None,
    };
    let root = temp.path().join("comparison");
    let report = campaign::run(&suite(), &base, Some(&candidate), &root, &options, false)
        .await
        .unwrap();
    assert_eq!(report["decision"]["retain"], true);
    assert_eq!(report["rows"].as_array().unwrap().len(), 6);
    assert_eq!(report["total_charged_tokens"], 450);
    let again = campaign::run(&suite(), &base, Some(&candidate), &root, &options, false)
        .await
        .unwrap();
    assert_eq!(again, report);
    assert_eq!(a.state.lock().unwrap().turns, 3);
    assert_eq!(b.state.lock().unwrap().turns, 3);
}
#[tokio::test]
async fn no_budget_and_changed_inputs_stop_before_business_submission() {
    let f = support::Fixture::start(100, json!({})).await;
    let temp = tempfile::tempdir().unwrap();
    let auth = temp.path().join("auth.json");
    f.auth(&auth);
    let port = Harness::new(&auth, 1000, 30).unwrap();
    let options = campaign::Options {
        repeats: 3,
        max_tokens: 999,
        run_reservation: 1000,
        input_root: None,
    };
    assert!(
        campaign::run(
            &suite(),
            &port,
            Some(&port),
            &temp.path().join("case"),
            &options,
            false
        )
        .await
        .is_err()
    );
    assert_eq!(f.state.lock().unwrap().turns, 0);
    let fp = port.fingerprint().await.unwrap();
    let root = temp.path().join("saved");
    port.execute(&root, &["amount".into()], &fp, &[])
        .await
        .unwrap();
    assert!(
        port.execute(&root, &["changed".into()], &fp, &[])
            .await
            .is_err()
    );
    assert_eq!(f.state.lock().unwrap().turns, 1);
}
#[test]
fn reviews_are_bound_to_reports_and_cannot_promote_a_failure() {
    let report = json!({"repeats":3,"learning_tokens":0,"expected_tasks":["t"],"rows":[{"key":"k","mode":"candidate","task_id":"t","attempt":0,"tokens":1,"delivered":false,"review_pending":true,"state":"failed","runtime_fingerprint":"x"}],"decision":{"reasons":["runtime_drift"]}});
    let review = json!({"report_sha256":exoharness_benchmark::digest(&report),"reviewer":"qa","decisions":[{"key":"k","passed":true}]});
    let result = campaign::review(&report, &review).unwrap();
    assert_eq!(result["rows"][0]["delivered"], false);
    assert_eq!(result["decision"]["retain"], false);
    let mut wrong = review;
    wrong["report_sha256"] = json!("bad");
    assert!(campaign::review(&report, &wrong).is_err());
}

#[tokio::test]
#[ignore = "requires an owned server fixture and explicit model-request opt-in"]
async fn owned_live_adapter_probe() {
    use exoharness_benchmark::{digest, evidence};
    assert_eq!(
        std::env::var("EXOHARNESS_PARITY_ALLOW_MODEL_REQUESTS").as_deref(),
        Ok("1")
    );
    let specpath = std::path::PathBuf::from(std::env::var("EXOHARNESS_PARITY_SPEC").unwrap());
    let spec = evidence::read_private(&specpath).unwrap();
    let auth = std::path::PathBuf::from(spec["auth_file"].as_str().unwrap());
    let root = std::path::PathBuf::from(spec["output"].as_str().unwrap());
    let limit = spec["reservation"].as_u64().unwrap();
    let port = Harness::new(&auth, limit, 120).unwrap();
    let fp = port.fingerprint().await.unwrap();
    let identity = json!({"prompt_sha256":digest(&spec["prompt"]),"runtime_fingerprint":fp});
    let path = root.join("ledger.json");
    let mut state = campaign::load_state(&path, identity).unwrap();
    let options = campaign::Options {
        repeats: 1,
        max_tokens: limit,
        run_reservation: limit,
        input_root: None,
    };
    options.validate(&port).unwrap();
    let outcome = campaign::execute_charged(
        &mut state,
        &path,
        "probe",
        &port,
        &[spec["prompt"].as_str().unwrap().to_string()],
        &fp,
        &[],
        &options,
    )
    .await
    .unwrap();
    evidence::write(&root.join("probe-report.json"), &outcome).unwrap();
}
