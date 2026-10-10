use super::*;
fn task() -> Value {
    json!({"version":1,"id":"owned-task","status":"working","poll":{"tool":"status","arguments":{"id":"owned-task"}},"cancel":{"tool":"cancel","arguments":{"id":"owned-task"}},"poll_interval_ms":2500})
}
#[test]
fn nested_envelope_and_identical_dual_declarations_are_equivalent() {
    let raw = task();
    for value in [
        json!({"_meta":{"exoharness":{"background_task":raw}}}),
        json!({"structuredContent":{"_meta":{"exoharness":{"background_task":raw}}}}),
        json!({"_meta":{"exoharness":{"background_task":raw}},"structuredContent":{"_meta":{"exoharness":{"background_task":raw}}}}),
    ] {
        let declared = declared(&value).unwrap().unwrap();
        assert_eq!(declared.id, "owned-task");
        assert_eq!(declared.poll.tool, "status");
        assert_eq!(declared.cancel.unwrap().tool, "cancel");
    }
}
#[test]
fn contradictory_or_invalid_nested_metadata_is_never_ignored() {
    let raw = task();
    let mut changed = raw.clone();
    changed["id"] = json!("other-task");
    assert!(declared(&json!({"_meta":{"exoharness":{"background_task":raw}},"structuredContent":{"_meta":{"exoharness":{"background_task":changed}}}})).is_err());
    for (field, value) in [
        ("version", json!(2)),
        ("status", json!("unknown")),
        ("poll", json!({"tool":"status","arguments":"{}"})),
    ] {
        let mut changed = raw.clone();
        changed[field] = value;
        assert!(
            declared(
                &json!({"structuredContent":{"_meta":{"exoharness":{"background_task":changed}}}})
            )
            .is_err()
        );
    }
    assert!(
        declared(&json!({"structuredContent":{"status":"queued"}}))
            .unwrap()
            .is_none()
    );
}

#[test]
fn cancellation_releases_only_an_explicit_terminal_receipt_for_the_owned_job() {
    for status in ["completed", "failed", "cancelled"] {
        let mut raw = task();
        raw["status"] = json!(status);
        let value = json!({"_meta":{"exoharness":{"background_task":raw}},"content":[]});
        let result = confirmed_terminal(value, "owned-task").unwrap();
        assert_eq!(result["isError"] == true, status != "completed");
    }
    for status in ["queued", "working", "running"] {
        let mut raw = task();
        raw["status"] = json!(status);
        assert!(
            confirmed_terminal(
                json!({"_meta":{"exoharness":{"background_task":raw}}}),
                "owned-task"
            )
            .is_none()
        );
    }
    let mut other = task();
    other["status"] = json!("cancelled");
    other["id"] = json!("other-task");
    assert!(
        confirmed_terminal(
            json!({"_meta":{"exoharness":{"background_task":other}}}),
            "owned-task"
        )
        .is_none()
    );
    assert!(
        confirmed_terminal(json!({"accepted":true,"status":"cancelled"}), "owned-task").is_none()
    );
}
