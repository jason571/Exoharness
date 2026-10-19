#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""GDPevo released-data adapter and isolated official grading."""
import argparse
import json
from pathlib import Path
import sys
root = Path(__file__).resolve().parents[1]
for module in ('benchmark', 'evolution'):
    sys.path.insert(0, str(root / 'modules' / module / 'src'))
from exoharness_benchmark.gdpevo import Dataset, OciGrader, run
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.harness import Harness


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('command', choices=('inspect', 'input', 'environment', 'grade', 'run'))
    p.add_argument('--checkout', required=True); p.add_argument('--revision', required=True)
    p.add_argument('--group'); p.add_argument('--split', choices=('train', 'test'), default='test')
    p.add_argument('--task'); p.add_argument('--environment-origin')
    p.add_argument('--prediction'); p.add_argument('--grading-image'); p.add_argument('--output', required=True)
    p.add_argument('--auth'); p.add_argument('--repeats', type=int, default=3)
    p.add_argument('--max-tokens', type=int, default=2000000)
    p.add_argument('--run-reservation', type=int, default=300000)
    p.add_argument('--allow-model-requests', action='store_true')
    p.add_argument('--owned-environment', action='store_true', help='Environment is disposable and exclusively owned by this run')
    a = p.parse_args(); dataset = Dataset(a.checkout, a.revision)
    if a.command == 'inspect':
        value = dataset.inventory()
    elif a.command == 'environment':
        value = dataset.environment_contract(a.group or '')
    elif a.command == 'input':
        value = dataset.solver_input(a.group or '', a.split, a.task or '', a.environment_origin or '')
    elif a.command == 'grade':
        if not a.prediction or not a.grading_image:
            p.error('grade requires a prediction and a pinned grading image')
        value = OciGrader(dataset, a.grading_image).grade(a.group or '', a.split, a.task or '',
            json.loads(Path(a.prediction).read_text()), a.output)
    else:
        if not a.allow_model_requests or not a.auth or not a.grading_image or not a.environment_origin:
            p.error('run requires --allow-model-requests, --auth, --grading-image and --environment-origin')
        value = run(dataset, a.group or '', Harness(a.auth, run_ceiling=a.run_reservation),
            OciGrader(dataset, a.grading_image), a.output, environment_url=a.environment_origin,
            repeats=a.repeats, max_tokens=a.max_tokens, owned_environment=a.owned_environment)
        print(json.dumps({'summary': value['summary'], 'charged_tokens': value['charged_tokens']}))
        return 0
    write_private(a.output, value)
    if a.command == 'inspect':
        print(json.dumps({'tasks': len(value['tasks']), 'dataset_sha256': value['dataset_sha256']}))
    return 0
if __name__ == '__main__':
    raise SystemExit(main())
