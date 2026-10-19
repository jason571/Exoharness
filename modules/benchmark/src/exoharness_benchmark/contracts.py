# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Frozen benchmark inputs and scoped experience, independent of adapters."""
from dataclasses import dataclass
import hashlib
import json
import re
import math


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
        separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def identifier(value):
    return isinstance(value, str) and re.fullmatch(r"[a-z][a-z0-9_-]{0,79}", value)


@dataclass(frozen=True)
class Suite:
    # Canonical JSON prevents mutation of a nested caller-owned dictionary.
    canonical: str

    @classmethod
    def parse(cls, value):
        if value.get("version") != 1 or not identifier(value.get("id")):
            raise ValueError("Invalid suite version or identity")
        tasks = value.get("tasks", [])
        if not tasks or len(tasks) > 1000:
            raise ValueError("Suite requires 1..1000 tasks")
        identities, prompts, groups, combinations = set(), set(), {}, {}
        for task in tasks:
            key, group, split = task.get("id"), task.get("group"), task.get("split")
            if not identifier(key) or key in identities or not identifier(group) or split not in ("train", "validation", "test", "regression"):
                raise ValueError("Invalid or duplicate task identity/split")
            turns = task.get("turns")
            if not isinstance(turns, list) or not 1 <= len(turns) <= 8 or not all(isinstance(t, str) and t.strip() and len(t) <= 16000 for t in turns):
                raise ValueError("Each task requires 1..8 bounded user turns")
            signature = digest([" ".join(t.split()).casefold() for t in turns])
            if signature in prompts:
                raise ValueError("Duplicate task input across the suite")
            checks = task.get("checks")
            evidence = task.get('evidence')
            if evidence is not None:
                permitted = {'input_sha256', 'source_trace_sha256', 'state', 'verified_delivery', 'failure_classes'}
                if (split != 'train' or not isinstance(evidence, dict) or set(evidence) != permitted
                    or evidence['input_sha256'] != digest(turns)
                    or not isinstance(evidence['source_trace_sha256'], str)
                    or not re.fullmatch(r'[0-9a-f]{64}', evidence['source_trace_sha256'])
                    or evidence['state'] not in ('succeeded', 'failed', 'recoverable', 'cancelled')
                    or type(evidence['verified_delivery']) is not bool
                    or not isinstance(evidence['failure_classes'], list)
                    or not all(c in ('input_invalid', 'transport', 'unknown_submission', 'capability_denied', 'budget', 'delivery_invalid') for c in evidence['failure_classes'])):
                    raise ValueError('Train evidence must be reviewed, input-matched, bounded Trace metadata')
            if 'review_required' in task and type(task['review_required']) is not bool:
                raise ValueError('Review requirement must be a boolean')
            for item in task.get("attachments", []):
                if (item.get('type') not in ('audio', 'image', 'file')
                    or not isinstance(item.get('path'), str)
                    or not re.fullmatch(r'inputs/[a-zA-Z0-9_.-]+', item['path'])
                    or not isinstance(item.get('sha256'), str)
                    or not re.fullmatch(r'[0-9a-f]{64}', item['sha256'])):
                    raise ValueError('Attachments require a pinned local input identity')
            if not isinstance(checks, list) or not checks:
                raise ValueError("A task requires executable delivery checks")
            for check in checks:
                kind = check.get("kind")
                allowed = {'json_equals': {'kind', 'fields'}, 'text_contains': {'kind', 'values'},
                    'tool_count': {'kind', 'name_contains', 'max', 'min', 'successful_only'},
                    'artifact': {'kind', 'extension', 'min_duration', 'max_duration', 'slides', 'audio', 'contains'}}
                if kind not in allowed or set(check) - allowed[kind]:
                    raise ValueError('Unknown delivery check or check field')
                if kind == "json_equals":
                    if not isinstance(check.get("fields"), dict) or not check["fields"]:
                        raise ValueError("JSON checks require expected fields")
                elif kind == "text_contains":
                    if not isinstance(check.get("values"), list) or not check["values"] or not all(isinstance(s, str) and s for s in check["values"]):
                        raise ValueError("Text checks require literal values")
                elif kind == "artifact":
                    if check.get("extension") not in (".html", ".zip", ".pptx", ".mp4", ".mp3", ".wav"):
                        raise ValueError("Unsupported delivery type")
                    for duration_key in ('min_duration', 'max_duration'):
                        if duration_key in check and (type(check[duration_key]) not in (int, float) or not math.isfinite(check[duration_key]) or check[duration_key] <= 0):
                            raise ValueError('Invalid duration')
                    if check.get('max_duration', float('inf')) < check.get('min_duration', 0):
                        raise ValueError('Duration bounds are reversed')
                    if 'slides' in check and not integer(check['slides'], 1):
                        raise ValueError('Slide count must be a positive integer')
                    if 'audio' in check and type(check['audio']) is not bool:
                        raise ValueError('Audio requirement must be a boolean')
                    if 'contains' in check and (not isinstance(check['contains'], list) or not all(isinstance(s, str) and s for s in check['contains'])):
                        raise ValueError('Artifact text checks require literal strings')
                elif kind == "tool_count":
                    if not isinstance(check.get("name_contains"), str) or not check["name_contains"] or not integer(check.get("max")):
                        raise ValueError("Invalid tool-call check")
                    if not integer(check.get('min', 0)) or check.get('min', 0) > check['max'] or type(check.get('successful_only', False)) is not bool:
                        raise ValueError('Invalid tool-call lower bound or success filter')
                else:
                    raise ValueError("Unknown delivery check")
            identities.add(key)
            prompts.add(signature)
            groups.setdefault(group, set()).add(split)
            if 'rule_ids' in task:
                declared = value.get('rules', {}).get(group, [])
                selected = task['rule_ids']
                if not isinstance(selected, list) or not selected or not all(identifier(r) and r in declared for r in selected) or len(selected) != len(set(selected)):
                    raise ValueError('Task rule combination is not declared for its group')
                combinations.setdefault(group, {}).setdefault(split, []).append(frozenset(selected))
        if any(not {"train", "test"} <= splits for splits in groups.values()):
            raise ValueError("Each group needs separate train and held-out tasks")
        for parts in combinations.values():
            train = parts.get('train', [])
            known = set().union(*train) if train else set()
            if any(combo in train or not combo <= known for split in ('test', 'validation') for combo in parts.get(split, [])):
                raise ValueError('Held-out combinations must recombine training rules without repeating a train combination')
        return cls(json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False))

    @property
    def value(self):
        return json.loads(self.canonical)

    @property
    def sha256(self):
        return digest(self.value)

    def training_material(self):
        # No test inputs, rubrics, scores, or prior test traces reach the learner.
        return [{"id": t["id"], "group": t["group"], "turns": t["turns"],
                 "expected_delivery": t["checks"], "reviewed_evidence": t.get('evidence'), "inputs": [
                     {k: a[k] for k in ('type', 'path', 'sha256')} for a in t.get('attachments', [])]}
                for t in self.value["tasks"] if t["split"] == "train"]


@dataclass(frozen=True)
class Experience:
    canonical: str

    @classmethod
    def parse(cls, value, suite):
        if value.get("version") != 1 or value.get("suite_sha256") != suite.sha256:
            raise ValueError("Candidate does not match the frozen suite")
        source = {t["id"]: t for t in suite.training_material()}
        rules = value.get("rules")
        if not isinstance(rules, list) or not 1 <= len(rules) <= 12:
            raise ValueError("Candidate requires 1..12 bounded rules")
        for rule in rules:
            if not identifier(rule.get("group")) or not isinstance(rule.get("sources"), list) or not rule["sources"]:
                raise ValueError("Experience needs a group and train provenance")
            if any(s not in source or source[s]["group"] != rule["group"] for s in rule["sources"]):
                raise ValueError("Experience source is not a matching training task")
            for field in ("when", "action", "unless"):
                if not isinstance(rule.get(field), str) or not rule[field].strip() or len(rule[field]) > 600:
                    raise ValueError("Experience requires bounded applicability, action and exception")
        if len(json.dumps(value, ensure_ascii=False)) > 12000:
            raise ValueError("Candidate instruction artifact is too large")
        return cls(json.dumps(value, sort_keys=True, ensure_ascii=False))

    @property
    def value(self):
        return json.loads(self.canonical)

    @property
    def sha256(self):
        return digest(self.value)

    def instructions(self, group):
        rules = [r for r in self.value["rules"] if r["group"] == group]
        if not rules:
            return ""
        return ("Task-scoped candidate guidance; current user requirements, live tool schemas, "
                "authorization and verified receipts remain authoritative. This does not grant "
                "capabilities or assert that an action has executed.\n" +
                "\n".join(f"When: {r['when']}\nAction: {r['action']}\nExcept: {r['unless']}" for r in rules))
