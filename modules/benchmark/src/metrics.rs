// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn usage(calls: &[Value]) -> Value {
    let (mut input, mut output, mut cached, mut unknown) = (0u64, 0u64, 0u64, 0u64);
    for call in calls {
        let v = &call["usage"];
        let cache = v
            .get("cache_read_tokens")
            .or(v.get("cached_input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if v.get("cache_read_tokens")
            .or(v.get("cached_input_tokens"))
            .is_some_and(|v| !v.is_null() && v.as_u64().is_none())
        {
            unknown += 1;
            continue;
        }
        match (v["input_tokens"].as_u64(), v["output_tokens"].as_u64()) {
            (Some(i), Some(o))
                if call["usage_reported"] == true
                    && cache <= i
                    && i.checked_add(o).is_some_and(|sum| {
                        v.get("total_tokens")
                            .is_none_or(|n| n.as_u64() == Some(sum))
                    }) =>
            {
                match (
                    input.checked_add(i),
                    output.checked_add(o),
                    cached.checked_add(cache),
                ) {
                    (Some(a), Some(b), Some(c)) => {
                        input = a;
                        output = b;
                        cached = c;
                    }
                    _ => unknown += 1,
                }
            }
            _ => unknown += 1,
        }
    }
    json!({"input_tokens":input,"output_tokens":output,"cached_input_tokens":cached,"total_tokens":input.saturating_add(output),"unknown_model_calls":unknown,"model_requests":calls.len()})
}
pub fn aggregate(rows: &[Value]) -> Value {
    let mut amounts: Vec<_> = rows.iter().filter_map(|r| r["tokens"].as_u64()).collect();
    amounts.sort();
    let mut times: Vec<_> = rows
        .iter()
        .filter_map(|r| r["elapsed_ms"].as_u64())
        .collect();
    times.sort();
    let successes = rows.iter().filter(|r| r["delivered"] == true).count();
    let complete = amounts.len() == rows.len();
    let total: u64 = amounts.iter().copied().sum();
    let median = if amounts.is_empty() || !complete {
        None
    } else {
        Some((amounts[(amounts.len() - 1) / 2] as f64 + amounts[amounts.len() / 2] as f64) / 2.0)
    };
    json!({"attempts":rows.len(),"deliveries":successes,"delivery_rate":if rows.is_empty(){0.0}else{successes as f64/rows.len() as f64},"first_turn_deliveries":rows.iter().filter(|r|r["first_turn_delivered"]==true).count(),"reported_tokens":total,"unknown_usage_attempts":rows.len()-amounts.len(),"median_tokens":median,"p95_tokens":if complete&&!amounts.is_empty(){Some(amounts[(amounts.len()*95).div_ceil(100)-1])}else{None},"max_tokens":if complete{amounts.last().copied()}else{None},"tokens_per_delivery":if complete&&successes>0{Some(total as f64/successes as f64)}else{None},"mean_elapsed_ms":if times.len()==rows.len()&&!times.is_empty(){Some(times.iter().sum::<u64>() as f64/times.len() as f64)}else{None},"p95_elapsed_ms":if times.len()==rows.len()&&!times.is_empty(){Some(times[(times.len()*95).div_ceil(100)-1])}else{None}})
}
pub struct Retention<'a> {
    pub tasks: &'a [String],
    pub repeats: u64,
    pub learning_tokens: Option<u64>,
    pub min_savings: f64,
    pub runtimes: Option<&'a Value>,
}
pub fn retain(rows: &[Value], options: Retention<'_>) -> Value {
    let mut reasons = BTreeSet::new();
    let mut observed = BTreeSet::new();
    let mut runs = BTreeSet::new();
    let mut paired = BTreeMap::<String, [usize; 2]>::new();
    let expected: BTreeSet<_> = ["base", "candidate"]
        .into_iter()
        .flat_map(|m| {
            options
                .tasks
                .iter()
                .flat_map(move |t| (0..options.repeats).map(move |a| (m.to_string(), t.clone(), a)))
        })
        .collect();
    let mut fingerprints = BTreeSet::new();
    for row in rows {
        let mode = row["mode"].as_str().unwrap_or("");
        let task = row["task_id"].as_str().unwrap_or("");
        let attempt = row["attempt"].as_u64().unwrap_or(u64::MAX);
        if !observed.insert((mode.to_string(), task.to_string(), attempt)) {
            reasons.insert("duplicate_attempt");
        }
        if let Some(ids) = row["run_ids"].as_array() {
            for id in ids {
                if !runs.insert(id.to_string()) {
                    reasons.insert("reused_run");
                }
            }
        }
        if row["delivered"] == true {
            paired.entry(task.to_string()).or_default()[usize::from(mode == "candidate")] += 1;
        }
        fingerprints.insert(row["runtime_fingerprint"].clone().to_string());
        if row["runtime_drift"] == true
            || options
                .runtimes
                .is_some_and(|p| p[mode].is_null() || p[mode] != row["runtime_fingerprint"])
        {
            reasons.insert("runtime_drift");
        }
        if row["tokens"].as_u64().is_none() {
            reasons.insert("unknown_usage");
        }
        if ["unknown", "running", "queued", "pending"]
            .contains(&row["state"].as_str().unwrap_or("unknown"))
        {
            reasons.insert("unresolved_execution");
        }
        if row["review_pending"] == true {
            reasons.insert("delivery_review_pending");
        }
    }
    if options.repeats < 3 {
        reasons.insert("insufficient_repeats");
    }
    if observed != expected {
        reasons.insert("incomplete_or_unexpected_matrix");
    }
    if options.learning_tokens.is_none() {
        reasons.insert("unknown_usage");
    }
    if options.runtimes.is_none() && (fingerprints.len() != 1 || fingerprints.contains("null")) {
        reasons.insert("runtime_drift");
    }
    if paired.values().any(|p| p[1] < p[0]) {
        reasons.insert("task_regression");
    }
    let base = aggregate(
        &rows
            .iter()
            .filter(|r| r["mode"] == "base")
            .cloned()
            .collect::<Vec<_>>(),
    );
    let candidate = aggregate(
        &rows
            .iter()
            .filter(|r| r["mode"] == "candidate")
            .cloned()
            .collect::<Vec<_>>(),
    );
    let amortized = options.learning_tokens.and_then(|cost| {
        if candidate["deliveries"].as_u64().unwrap_or(0) > 0
            && candidate["unknown_usage_attempts"] == 0
        {
            Some(
                (candidate["reported_tokens"].as_u64().unwrap_or(0) as f64 + cost as f64)
                    / candidate["deliveries"].as_u64().unwrap_or(1) as f64,
            )
        } else {
            None
        }
    });
    let accuracy = candidate["deliveries"].as_u64() > base["deliveries"].as_u64();
    let gain = amortized
        .zip(base["tokens_per_delivery"].as_f64())
        .is_some_and(|(a, b)| a <= b * (1.0 - options.min_savings));
    if !accuracy && !gain {
        reasons.insert("no_measured_gain_including_learning");
    }
    json!({"retain":reasons.is_empty(),"reasons":reasons,"base":base,"candidate":candidate,"learner_tokens":options.learning_tokens,"candidate_tokens_per_delivery_including_learning":amortized,"scope":"local candidate artifact only; no production import or permission change"})
}
