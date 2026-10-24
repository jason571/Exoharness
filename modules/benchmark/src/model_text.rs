// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! Native rendering of the established model-facing JSON format.
//! Evidence digests use their separate compact canonical representation.
use serde_json::Value;

pub fn json(value: &Value) -> String {
    match value {
        Value::Array(values) => array(values.iter().map(json)),
        Value::Object(values) => object(
            &values
                .iter()
                .map(|(key, value)| (key.as_str(), json(value)))
                .collect::<Vec<_>>(),
        ),
        _ => crate::canonical::json(value),
    }
}
pub fn array(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(", "))
}
pub fn object(fields: &[(&str, String)]) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(key, value)| format!(
                "{}: {value}",
                serde_json::to_string(key).expect("JSON key")
            ))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
pub fn selected(value: &Value, fields: &[&str]) -> String {
    object(
        &fields
            .iter()
            .filter_map(|key| value.get(key).map(|v| (*key, json(v))))
            .collect::<Vec<_>>(),
    )
}
pub fn training_material(values: &[Value]) -> String {
    array(values.iter().map(|value| {
        let mut fields = Vec::new();
        for key in [
            "id",
            "group",
            "turns",
            "expected_delivery",
            "reviewed_evidence",
            "inputs",
        ] {
            if let Some(v) = value.get(key) {
                let rendered = if key == "inputs" {
                    array(
                        v.as_array()
                            .into_iter()
                            .flatten()
                            .map(|input| selected(input, &["type", "path", "sha256"])),
                    )
                } else {
                    json(v)
                };
                fields.push((key, rendered));
            }
        }
        object(&fields)
    }))
}
pub fn trajectories(values: &[Value]) -> String {
    array(values.iter().map(|value| {
        let mut fields = Vec::new();
        for key in [
            "train_id",
            "verified_delivery",
            "state",
            "tokens",
            "steps",
            "failure_classes",
            "evidence_sha256",
        ] {
            if let Some(v) = value.get(key) {
                let rendered = if key == "steps" {
                    array(
                        v.as_array()
                            .into_iter()
                            .flatten()
                            .map(|step| selected(step, &["tool", "outcome"])),
                    )
                } else {
                    json(v)
                };
                fields.push((key, rendered));
            }
        }
        object(&fields)
    }))
}
