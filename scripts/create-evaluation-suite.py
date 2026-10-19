#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Create a frozen private task suite, without any service or model calls."""
import argparse
import json
import os
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'modules/benchmark/src'))
from exoharness_benchmark.scenarios import create

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--kind', choices=('workflows', 'text-smoke', 'current-reports'), required=True)
parser.add_argument('--output', required=True)
parser.add_argument('--mv-audio', help='Existing WAV for workflow MV tasks; preserved verbatim')
args = parser.parse_args()
os.umask(0o077)
try:
    suite = create(args.output, args.kind, args.mv_audio)
except ValueError as error:
    parser.error(str(error))
print(json.dumps({'suite_sha256': suite.sha256, 'tasks': len(suite.value['tasks']), 'model_requests': 0}))
