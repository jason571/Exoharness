# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Benchmark accounting and conservative retention, including failed attempts."""
from collections import defaultdict
import math
from .contracts import integer


def usage(calls):
    totals = {"input_tokens": 0, "output_tokens": 0, "cached_input_tokens": 0}
    unknown = 0
    for call in calls:
        value = call.get("usage") or {}
        incoming, outgoing = value.get("input_tokens"), value.get("output_tokens")
        cached = value.get("cache_read_tokens", value.get("cached_input_tokens"))
        cached = 0 if cached is None else cached
        if call.get("usage_reported") is not True or not integer(incoming) or not integer(outgoing) or not integer(cached) or cached > incoming:
            unknown += 1
            continue
        if value.get("total_tokens") is not None and value["total_tokens"] != incoming + outgoing:
            unknown += 1
            continue
        totals["input_tokens"] += incoming
        totals["output_tokens"] += outgoing
        totals["cached_input_tokens"] += cached
    return {**totals, "total_tokens": totals["input_tokens"] + totals["output_tokens"],
            "unknown_model_calls": unknown, "model_requests": len(calls)}


def aggregate(rows):
    measured = [r for r in rows if r.get("tokens") is not None]
    successes = sum(r.get("delivered") is True for r in rows)
    tokens = sum(r["tokens"] for r in measured)
    complete = len(measured) == len(rows)
    timings = sorted(r['elapsed_ms'] for r in rows if integer(r.get('elapsed_ms')))
    amounts = sorted(r['tokens'] for r in measured)
    median = ((amounts[(len(amounts) - 1) // 2] + amounts[len(amounts) // 2]) / 2 if amounts else None)
    return {"attempts": len(rows), "deliveries": successes,
            "delivery_rate": successes / len(rows) if rows else 0,
            "first_turn_deliveries": sum(r.get("first_turn_delivered") is True for r in rows),
            "reported_tokens": tokens, "unknown_usage_attempts": len(rows) - len(measured),
            "median_tokens": median if complete else None,
            "p95_tokens": amounts[math.ceil(len(amounts) * .95) - 1] if amounts and complete else None,
            "max_tokens": max(amounts) if amounts and complete else None,
            "tokens_per_delivery": tokens / successes if successes and complete else None,
            "mean_elapsed_ms": sum(timings) / len(timings) if timings and len(timings) == len(rows) else None,
            "p95_elapsed_ms": timings[math.ceil(len(timings) * .95) - 1] if timings and len(timings) == len(rows) else None}


def retain(rows, *, expected_tasks, repeats, learner_tokens, min_savings=0.05, expected_runtimes=None):
    reasons, paired = [], defaultdict(dict)
    expected = {(mode, task, attempt) for mode in ("base", "candidate") for task in expected_tasks for attempt in range(repeats)}
    observed = set()
    run_ids = set()
    for row in rows:
        key = row["mode"], row["task_id"], row["attempt"]
        if key in observed:
            reasons.append("duplicate_attempt")
        observed.add(key)
        for run in row.get("run_ids", []):
            if run in run_ids:
                reasons.append("reused_run")
            run_ids.add(run)
        paired[row["task_id"]][row["mode"]] = paired[row["task_id"]].get(row["mode"], 0) + (row.get("delivered") is True)
    if repeats < 3:
        reasons.append("insufficient_repeats")
    if observed != expected:
        reasons.append("incomplete_or_unexpected_matrix")
    if learner_tokens is None or any(r.get("tokens") is None for r in rows):
        reasons.append("unknown_usage")
    fingerprints = {r.get("runtime_fingerprint") for r in rows}
    valid_fingerprints = (all(r.get('runtime_fingerprint') == expected_runtimes.get(r['mode']) for r in rows)
                          and set(expected_runtimes) == {'base', 'candidate'} and all(expected_runtimes.values())
                          if expected_runtimes is not None else len(fingerprints) == 1 and None not in fingerprints)
    if not valid_fingerprints or any(r.get("runtime_drift") for r in rows):
        reasons.append("runtime_drift")
    if any(r.get("state") in ("unknown", "running", "queued", "pending") for r in rows):
        reasons.append("unresolved_execution")
    if any(r.get("review_pending") for r in rows):
        reasons.append("delivery_review_pending")
    if any(p.get("candidate", 0) < p.get("base", 0) for p in paired.values()):
        reasons.append("task_regression")
    base = aggregate([r for r in rows if r["mode"] == "base"])
    candidate = aggregate([r for r in rows if r["mode"] == "candidate"])
    amortized = ((candidate["reported_tokens"] + learner_tokens) / candidate["deliveries"]
                 if candidate["deliveries"] and learner_tokens is not None and not candidate["unknown_usage_attempts"] else None)
    accuracy_gain = candidate["deliveries"] > base["deliveries"]
    cost_gain = (amortized is not None and base["tokens_per_delivery"] is not None
                 and amortized <= base["tokens_per_delivery"] * (1 - min_savings))
    if not accuracy_gain and not cost_gain:
        reasons.append("no_measured_gain_including_learning")
    return {"retain": not reasons, "reasons": sorted(set(reasons)), "base": base,
            "candidate": candidate, "learner_tokens": learner_tokens,
            "candidate_tokens_per_delivery_including_learning": amortized,
            "scope": "local candidate artifact only; no production import or permission change"}
