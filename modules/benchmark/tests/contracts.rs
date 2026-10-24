use exoharness_benchmark::{Experience, Suite, evidence, metrics, scenarios, verifiers};
use serde_json::{Value, json};
#[test]
fn only_pinned_attachment_metadata_reaches_the_learner() {
    let mut value = scenarios::suite("text-smoke", None)
        .unwrap()
        .value()
        .clone();
    value["tasks"][0]["attachments"] = json!([{"type":"file","path":"inputs/example.txt","sha256":"a".repeat(64),"private_metadata":"NEVER_PASS_THIS_FIELD"}]);
    let suite = Suite::parse(value).unwrap();
    let material = suite.training_material();
    assert!(!material.to_string().contains("NEVER_PASS_THIS_FIELD"));
    assert_eq!(material[0]["inputs"][0].as_object().unwrap().len(), 3);
    assert!(exoharness_benchmark::flagged(&json!({"code":"rejected"})));
    assert!(!exoharness_benchmark::flagged(&json!({})));
}
#[test]
fn legacy_python_numeric_evidence_identities_remain_compatible() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../scripts/tests/fixtures/rust-evaluation/numeric-identities.json"
    ))
    .unwrap();
    for record in fixtures.as_array().unwrap() {
        assert_eq!(
            exoharness_benchmark::digest(&record["value"]),
            record["expected_sha256"],
            "{}",
            record["value"]
        );
    }
}
#[test]
fn canonical_digest_survives_fractional_metric_persistence() {
    for n in 1..2000 {
        let original = json!({"mean_elapsed_ms":n as f64/3.0});
        let persisted: Value =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(
            exoharness_benchmark::digest(&original),
            exoharness_benchmark::digest(&persisted),
            "metric roundtrip at {n}: {original} / {persisted}"
        );
    }
}
#[test]
fn python_suite_identities_and_training_partitions_are_preserved() {
    for (kind, sha, count) in [
        (
            "text-smoke",
            "1c349ef454f2869fd4987d70b16623063023620541bbdb31b002ba180cef02aa",
            4,
        ),
        (
            "workflows",
            "d08416390b07913980357ec9db7b48b247e11f25781520250206ff7b5f84c3f6",
            62,
        ),
        (
            "current-reports",
            "ee9df7fc3dd75c1fe51c2a394ee3372cbeea7ba1b92edc9f5db93d39c615cd20",
            3,
        ),
    ] {
        let suite = scenarios::suite(kind, Some(&"a".repeat(64))).unwrap();
        assert_eq!(suite.sha256(), sha);
        assert_eq!(suite.tasks().len(), count);
        assert!(
            suite
                .training_material()
                .as_array()
                .unwrap()
                .iter()
                .all(|t| !t["id"].as_str().unwrap().starts_with("test_")
                    && !t["id"].as_str().unwrap().starts_with("regression_"))
        );
    }
}
#[test]
fn invalid_suites_and_checks_are_rejected() {
    let original = scenarios::suite("text-smoke", None).unwrap();
    let variants = [
        ("duplicate", json!(null)),
        ("bool_version", json!(true)),
        ("bool_min", json!(true)),
        ("negative_min", json!(-1)),
        ("missing_checks", json!([])),
        ("attachment_escape", json!("inputs/../secret")),
        ("unknown_check", json!("run_shell")),
        ("reversed_duration", json!(null)),
    ];
    for (kind, value) in variants {
        let mut suite = original.value().clone();
        match kind {
            "duplicate" => suite["tasks"][3]["turns"] = suite["tasks"][0]["turns"].clone(),
            "bool_version" => suite["version"] = value,
            "bool_min" | "negative_min" => suite["tasks"][3]["checks"][1]["min"] = value,
            "missing_checks" => suite["tasks"][3]["checks"] = value,
            "attachment_escape" => {
                suite["tasks"][3]["attachments"] =
                    json!([{"type":"file","path":value,"sha256":"a".repeat(64)}])
            }
            "unknown_check" => suite["tasks"][3]["checks"][0]["kind"] = value,
            "reversed_duration" => {
                suite["tasks"][3]["checks"] = json!([{"kind":"artifact","extension":".mp4","min_duration":20,"max_duration":10}])
            }
            _ => unreachable!(),
        }
        assert!(Suite::parse(suite).is_err(), "{kind}");
    }
    assert_eq!(
        Suite::parse(original.value().clone()).unwrap().sha256(),
        original.sha256()
    );
}
#[test]
fn candidate_sources_cannot_use_test_or_foreign_tasks() {
    let suite = scenarios::suite("text-smoke", None).unwrap();
    let valid = json!({"version":1,"suite_sha256":suite.sha256(),"rules":[{"group":"math","sources":["train_math_0"],"when":"A units digit changed.","action":"Divide the difference by the digit delta.","unless":"Values are missing."}]});
    assert!(Experience::parse(valid.clone(), &suite).is_ok());
    for source in ["test_math_composition", "foreign"] {
        let mut invalid = valid.clone();
        invalid["rules"][0]["sources"] = json!([source]);
        assert!(Experience::parse(invalid, &suite).is_err());
    }
}
#[test]
fn tokens_count_cache_once_and_retain_unknown_usage() {
    let calls = json!([{"usage_reported":true,"usage":{"input_tokens":100,"output_tokens":5,"cache_read_tokens":80,"total_tokens":105}},{"usage_reported":true,"usage":{"input_tokens":20,"output_tokens":3}}]);
    let total = metrics::usage(calls.as_array().unwrap());
    assert_eq!(total["total_tokens"], 128);
    assert_eq!(total["cached_input_tokens"], 80);
    assert_eq!(total["unknown_model_calls"], 0);
    for value in [
        json!({}),
        json!({"usage_reported":false,"usage":{"input_tokens":1,"output_tokens":1}}),
        json!({"usage_reported":true,"usage":{"input_tokens":true,"output_tokens":1}}),
        json!({"usage_reported":true,"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":3}}),
        json!({"usage_reported":true,"usage":{"input_tokens":1,"output_tokens":1,"cache_read_tokens":2}}),
    ] {
        assert_eq!(metrics::usage(&[value])["unknown_model_calls"], 1);
    }
}
#[test]
fn successful_runtime_status_alone_does_not_pass_delivery() {
    let task =
        json!({"checks":[{"kind":"artifact","extension":".mp4","audio":true,"min_duration":10}]});
    for outcome in [
        json!({"state":"succeeded","artifacts":[]}),
        json!({"state":"failed","artifacts":[{"extension":".mp4","nonempty":true,"audio":true,"video":true,"duration":15}]}),
        json!({"state":"succeeded","artifacts":[{"extension":".mp4","nonempty":true,"video":true,"duration":15}]}),
    ] {
        assert_eq!(verifiers::verify(&task, &outcome)["delivered"], false);
    }
    assert_eq!(
        verifiers::verify(
            &task,
            &json!({"state":"succeeded","artifacts":[{"extension":".mp4","nonempty":true,"audio":true,"video":true,"duration":15}]})
        )["delivered"],
        true
    );
}
#[test]
fn exact_json_types_and_successful_tool_receipts_are_checked() {
    let task = json!({"checks":[{"kind":"json_equals","fields":{"amount":42}},{"kind":"tool_count","name_contains":"lookup","min":1,"max":2,"successful_only":true}]});
    let mut outcome = json!({"state":"succeeded","text":"{\"amount\":42}","tool_names":["lookup"],"successful_tool_names":[]});
    assert_eq!(verifiers::verify(&task, &outcome)["delivered"], false);
    outcome["successful_tool_names"] = json!(["lookup"]);
    assert_eq!(verifiers::verify(&task, &outcome)["delivered"], true);
    outcome["text"] = json!("{\"amount\":true}");
    assert_eq!(verifiers::verify(&task, &outcome)["delivered"], false);
}
#[test]
fn retention_requires_complete_repeats_known_usage_and_review() {
    let mut rows = Vec::new();
    for attempt in 0..3 {
        for (mode, tokens) in [("base", 100), ("candidate", 50)] {
            rows.push(json!({"mode":mode,"task_id":"task","attempt":attempt,"run_ids":[format!("{mode}-{attempt}")],"delivered":true,"tokens":tokens,"state":"succeeded","runtime_fingerprint":"same","runtime_drift":false}));
        }
    }
    let ids = vec!["task".into()];
    let check = |rows: &[Value], repeats, learning| {
        metrics::retain(
            rows,
            metrics::Retention {
                tasks: &ids,
                repeats,
                learning_tokens: learning,
                min_savings: 0.05,
                runtimes: None,
            },
        )
    };
    assert_eq!(check(&rows, 3, Some(0))["retain"], true);
    assert_eq!(check(&rows, 2, Some(0))["retain"], false);
    assert_eq!(check(&rows, 3, Some(1000))["retain"], false);
    for field in ["tokens", "review_pending", "runtime_drift", "delivered"] {
        let mut invalid = rows.clone();
        invalid[1][field] = match field {
            "tokens" => Value::Null,
            "delivered" => json!(false),
            _ => json!(true),
        };
        assert_eq!(check(&invalid, 3, Some(0))["retain"], false, "{field}");
    }
}
#[test]
fn evidence_locks_and_atomic_files_preserve_private_state() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let lock = evidence::Exclusive::acquire(root.path()).unwrap();
    assert!(evidence::Exclusive::acquire(root.path()).is_err());
    let path = root.path().join("state.json");
    evidence::write(&path, &json!({"attempt":1})).unwrap();
    assert_eq!(evidence::read_private(&path).unwrap()["attempt"], 1);
    drop(lock);
    assert!(evidence::Exclusive::acquire(root.path()).is_ok());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&path, root.path().join("link")).unwrap();
        assert!(evidence::read(&root.path().join("link")).is_err());
        assert!(evidence::write(&root.path().join("link"), &json!({})).is_err());
    }
}
#[test]
fn archive_and_slide_inspection_validate_actual_contents() {
    use std::io::{Cursor, Write};
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "ppt/slides/slide1.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(b"<p><t>AI &amp; Rust</t></p>").unwrap();
    let raw = writer.finish().unwrap().into_inner();
    let result = verifiers::inspect_artifact("lesson.pptx", &raw, None).unwrap();
    assert_eq!(result["slides"], 1);
    assert_eq!(result["text"], "AI & Rust");
    assert!(verifiers::inspect_artifact("bad.pptx", b"not a zip", None).is_err());
}

#[test]
fn malformed_slide_xml_does_not_become_a_valid_delivery() {
    use std::io::{Cursor, Write};
    for xml in [
        "<p><t>unclosed",
        "<a/><b/>",
        "<p><t>&unknown;</t></p>",
        "<p><t>mismatch</p>",
    ] {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "ppt/slides/slide1.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(xml.as_bytes()).unwrap();
        let raw = writer.finish().unwrap().into_inner();
        assert!(
            verifiers::inspect_artifact("bad.pptx", &raw, None).is_err(),
            "{xml}"
        );
    }
}
#[test]
fn official_score_formats_are_normalized_without_guessing() {
    for value in [
        json!({"normalized_score":0.5}),
        json!({"total_score":0.5}),
        json!({"score":5,"max_score":10}),
        json!({"score":0.5,"raw_score":5}),
    ] {
        assert_eq!(
            exoharness_benchmark::gdpevo::normalize_score(&value).unwrap(),
            0.5
        );
    }
    for invalid in [
        json!({"score":2}),
        json!({"score":true}),
        json!({"score":1,"max_score":0}),
    ] {
        assert!(exoharness_benchmark::gdpevo::normalize_score(&invalid).is_err());
    }
}
