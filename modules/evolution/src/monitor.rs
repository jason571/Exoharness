// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{ledger::Ledger, publisher::Publisher};
use exoharness_benchmark::{Result, encoded, ensure, evidence, list, metrics, text};
use serde_json::{Value, json};
pub async fn observe(publisher: &Publisher<'_>, ledger: &mut Ledger) -> Result<Value> {
    let publication = list(&ledger.state["publications"])?.last().cloned();
    let owners = list(&publisher.policy.value()["monitor_user_ids"])?;
    let Some(publication) = publication else {
        return Ok(json!({"status":"not_configured","samples":0}));
    };
    if owners.is_empty() {
        return Ok(json!({"status":"not_configured","samples":0}));
    }
    let revision = &publication["revision"];
    let key = revision.to_string();
    let mut observations = ledger.state["monitor"]
        .get(&key)
        .cloned()
        .unwrap_or(json!({}));
    for page in 0..2 {
        let sessions = publisher
            .client
            .api(
                &format!("/v1/trace/sessions?limit=50&offset={}", page * 50),
                None,
                "GET",
                false,
            )
            .await?;
        for session in list(&sessions)? {
            if !owners.contains(&session["owner_user_id"]) {
                continue;
            }
            let runs = publisher
                .client
                .api(
                    &format!(
                        "/v1/trace/runs?session_id={}&limit=50",
                        encoded(text(&session["id"])?)
                    ),
                    None,
                    "GET",
                    false,
                )
                .await?;
            for run in list(&runs)? {
                let id = text(&run["id"])?;
                if observations.get(id).is_some()
                    || !["succeeded", "failed", "recoverable"]
                        .contains(&run["status"].as_str().unwrap_or(""))
                {
                    continue;
                }
                let trace = publisher
                    .client
                    .api(
                        &format!("/v1/trace/runs/{}?event_limit=1", encoded(id)),
                        None,
                        "GET",
                        false,
                    )
                    .await?;
                if list(&trace["invocations"])?.iter().any(|i| {
                    i["tool_name"] == "read_skill"
                        && i["arguments"]["id"] == publisher.policy.skill_id()
                        && i["result"]["revision"] == *revision
                        && i["result"]["isError"] != true
                }) {
                    let usage = metrics::usage(list(&trace["model_calls"])?);
                    observations[id] = json!({"succeeded":run["status"]=="succeeded","tokens":if usage["unknown_model_calls"]==0{usage["total_tokens"].clone()}else{Value::Null},"created_at":run["created_at"].as_u64().unwrap_or(0)});
                }
            }
        }
        if list(&sessions)?.len() < 50 {
            break;
        }
    }
    let mut entries = observations
        .as_object()
        .expect("observation map")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|(id, v)| {
        (
            std::cmp::Reverse(v["created_at"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(id.clone()),
        )
    });
    entries.truncate(1000);
    observations = Value::Object(entries.into_iter().collect());
    let values = observations.as_object().expect("observations");
    let failures = values.values().filter(|v| v["succeeded"] != true).count();
    let known: Vec<_> = values
        .values()
        .filter_map(|v| v["tokens"].as_u64())
        .collect();
    let mean = if known.is_empty() {
        None
    } else {
        Some(known.iter().sum::<u64>() as f64 / known.len() as f64)
    };
    let regression = values.len() as u64 >= publisher.policy.n("monitor_min_samples")
        && (failures as f64 / values.len() as f64
            > publisher.policy.value()["monitor_failure_rate"]
                .as_f64()
                .expect("fraction")
            || known.len() != values.len()
            || mean.is_some_and(|n| n > publisher.policy.n("monitor_max_tokens") as f64));
    let mut result = json!({"status":if regression{"regression"}else{"observing"},"samples":values.len(),"failures":failures,"mean_tokens":mean,"skill_revision":revision,"suspended":false,"sampling":"bounded_recent_window"});
    ledger.state["monitor"][&key] = observations;
    ledger.save()?;
    if regression && publisher.policy.value()["auto_suspend"] == true {
        if ledger.state["suspensions"].is_null() {
            ledger.state["suspensions"] = json!({});
        }
        if ledger.state["suspensions"].get(&key).is_none() {
            ledger.state["suspensions"][&key] = json!({"revision":revision,"phase":"planned"});
        }
        let mut record = publisher.record().await?;
        ensure(
            record.is_null() || record["source_digest"] == publication["identity"]["source_digest"],
            "Refuse to suspend unrelated package",
        )?;
        if record["enabled"] == true {
            ensure(
                record["revision"] == *revision,
                "Published revision changed",
            )?;
            ledger.state["suspensions"][&key]["phase"] = json!("submitting");
            ledger.save()?;
            record = publisher
                .suspend(revision, &publication["identity"]["source_digest"])
                .await?;
        }
        if !record.is_null() && record["enabled"] == false {
            ledger.state["suspensions"][&key]["phase"] = json!("confirmed");
            ledger.state["suspensions"][&key]["disabled_revision"] = record["revision"].clone();
            ledger.save()?;
            result["suspended"] = json!(true);
        }
    }
    evidence::write(&ledger.root.join("monitor.json"), &result)?;
    Ok(result)
}
