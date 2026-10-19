# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Offline entry point; never imported by the online Agent execution loop."""
import argparse
import json
from pathlib import Path
import time
from exoharness_benchmark.contracts import Suite, identifier
from exoharness_benchmark.harness import Harness
from .contracts import Policy
from .controller import Controller
from .publisher import Management
from .queue import tick


def load(path):
    return json.loads(Path(path).read_text())


def factory(job, output):
    policy = Policy.parse(load(job['policy']))
    return Controller(policy, Suite.parse(load(job['suite'])),
        Harness(job['baseline_auth'], run_ceiling=policy.value['run_reservation']),
        Harness(job['canary_auth'], run_ceiling=policy.value['run_reservation']),
        Management(job['publication_auth']), Management(job['canary_auth']), output,
        input_root=job['input_root'], training_evidence=job.get('training_evidence'))


def main(argv=None):
    parser = argparse.ArgumentParser(description='Bounded, isolated Skill experience evaluation and publication')
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('skill-package', help='Package skills/workflow-evolution for the normal Skill import API')
    p.add_argument('--output', required=True)
    for command in ('run', 'publish', 'monitor', 'status'):
        p = sub.add_parser(command)
        p.add_argument('--job', required=True)
        p.add_argument('--state-root', required=True)
        if command == 'run':
            p.add_argument('--allow-model-requests', action='store_true')
    p = sub.add_parser('watch')
    p.add_argument('--queue', required=True); p.add_argument('--state-root', required=True)
    p.add_argument('--max-tokens', required=True, type=int)
    p.add_argument('--interval-seconds', type=int, default=3600)
    p.add_argument('--cycles', type=int, default=1, help='0 runs until stopped; budgets remain finite')
    p.add_argument('--allow-model-requests', action='store_true')
    args = parser.parse_args(argv)
    if args.command == 'skill-package':
        from .skill import package
        from exoharness_benchmark.evidence import write_private
        write_private(args.output, package())
        print(json.dumps({'output': args.output, 'skill': 'workflow-evolution'}))
        return 0
    if args.command in ('watch', 'run') and not args.allow_model_requests:
        parser.error('Model evaluation requires --allow-model-requests')
    if args.command == 'watch':
        if args.interval_seconds < 60 or args.cycles < 0 or args.max_tokens < 1:
            parser.error('Positive Token limit, interval >=60 and cycles >=0 required')
        cycle = 0
        while args.cycles == 0 or cycle < args.cycles:
            result = tick(args.queue, args.state_root, args.max_tokens, factory)
            print(json.dumps(result, ensure_ascii=False), flush=True)
            cycle += 1
            if args.cycles == 0 or cycle < args.cycles:
                # Short sleeps permit normal shutdown and interactive steering.
                deadline = time.monotonic() + args.interval_seconds
                while time.monotonic() < deadline:
                    time.sleep(min(30, deadline - time.monotonic()))
        return 0
    job = load(args.job)
    if not identifier(job.get('id')):
        parser.error('Invalid campaign identity')
    output = Path(args.state_root) / job['id']
    if args.command == 'status':
        state = load(output / 'evolution.json')
        print(json.dumps({'phase': state['phase'], 'charged_tokens': sum(state['charges'].values()),
                          'rounds': state['rounds'], 'publications': state['publications']}, ensure_ascii=False))
        return 0
    controller = factory(job, output)
    result = getattr(controller, args.command)()
    print(json.dumps(result, ensure_ascii=False))
    return 0
