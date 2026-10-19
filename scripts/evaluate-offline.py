#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Plan or run a frozen offline experience experiment through ExoHarness.

Example: python3 scripts/evaluate-offline.py plan --suite private-suite.json
Run additionally requires --allow-model-requests, --auth-file and --output.
Each attempt reserves the configured server Run Token ceiling before submission.
The learner sees training tasks only. Candidate guidance is scoped inline text;
no shared Skill, account, permission, model configuration or service is updated.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'modules/benchmark/src'))
from exoharness_benchmark.contracts import Suite
from exoharness_benchmark.experiment import Experiment
from exoharness_benchmark.harness import Harness
from exoharness_benchmark.selection import review
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.comparison import RuntimeComparison
from exoharness_benchmark.acceptance import Acceptance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('plan', 'run', 'review', 'compare', 'accept'))
    parser.add_argument('--suite')
    parser.add_argument('--report', help='Original report for a local delivery review')
    parser.add_argument('--reviews', help='Reviewer decisions bound to the original report SHA-256')
    parser.add_argument('--allow-model-requests', action='store_true')
    parser.add_argument('--auth-file')
    parser.add_argument('--candidate-auth-file', help='Separate isolated candidate for a runtime comparison')
    parser.add_argument('--output')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--max-tokens', type=int, default=1000000)
    parser.add_argument('--run-reservation', type=int, default=300000,
                        help='At least the server Run Token stopping threshold; unknown usage retains it')
    args = parser.parse_args()
    if args.action in ('run', 'compare', 'accept') and not args.allow_model_requests:
        parser.error('run requires --allow-model-requests before credentials or network access')
    if args.action in ('run', 'compare', 'accept') and (not args.auth_file or not args.output):
        parser.error('run requires --auth-file and --output')
    if args.action == 'review':
        if not args.report or not args.reviews or not args.output:
            parser.error('review requires --report, --reviews and a new --output file')
        if Path(args.output).exists():
            parser.error('review output must not replace existing evidence')
        try:
            result = review(json.loads(Path(args.report).read_text()), json.loads(Path(args.reviews).read_text()))
            write_private(args.output, result)
        except (ValueError, RuntimeError) as error:
            parser.exit(1, str(error) + '\n')
        print(json.dumps(result['decision'], indent=2))
        return
    if not args.suite:
        parser.error('plan and run require --suite')
    if args.action == 'compare' and not args.candidate_auth_file:
        parser.error('compare requires --candidate-auth-file')
    suite = Suite.parse(json.loads(Path(args.suite).read_text()))
    if args.action == 'plan':
        print(json.dumps({'suite': suite.value['id'], 'sha256': suite.sha256,
            'train_tasks': len(suite.training_material()),
            'held_out_tasks': sum(t['split'] == 'test' for t in suite.value['tasks']),
            'regression_tasks': sum(t['split'] == 'regression' for t in suite.value['tasks']),
            'solver_attempts': sum(t['split'] != 'train' for t in suite.value['tasks']) * args.repeats * 2,
            'model_requests_submitted': 0}, indent=2))
        return
    os.umask(0o077)
    root = Path(args.output)
    if root.is_symlink():
        parser.error('Output directory cannot be a symlink')
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    root.chmod(0o700)
    with (root / '.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            parser.error('Another evaluator owns this experiment')
        try:
            port = Harness(args.auth_file, run_ceiling=args.run_reservation)
            options = dict(repeats=args.repeats, max_tokens=args.max_tokens,
                           run_reservation=args.run_reservation, input_root=Path(args.suite).resolve().parent)
            if args.action == 'accept':
                report = Acceptance(suite, port, root, **options).execute()
            elif args.action == 'compare':
                candidate = Harness(args.candidate_auth_file, run_ceiling=args.run_reservation)
                report = RuntimeComparison(suite, port, candidate, root, **options).execute()
            else:
                report = Experiment(suite, port, root, **options).execute()
        except (ValueError, RuntimeError) as error:
            parser.exit(1, str(error) + '\n')
    print(json.dumps({k: report.get(k) for k in ('suite_sha256', 'candidate_sha256',
        'total_charged_tokens', 'learning_tokens', 'decision', 'metrics', 'all_structural_checks_passed', 'review_pending', 'production_configuration_modified')}, indent=2))


if __name__ == '__main__':
    main()
