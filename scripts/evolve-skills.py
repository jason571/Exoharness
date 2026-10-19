#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Run the independent experience evolution module from a source checkout."""
from pathlib import Path
import sys
root = Path(__file__).resolve().parents[1]
for module in ('benchmark', 'evolution'):
    sys.path.insert(0, str(root / 'modules' / module / 'src'))
from exoharness_evolution.cli import main
if __name__ == '__main__':
    raise SystemExit(main())
