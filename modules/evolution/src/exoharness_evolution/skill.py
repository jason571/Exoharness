# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Load and package the independently installed workflow-evolution Skill."""
import base64
import hashlib
from pathlib import Path


def instructions():
    path = Path(__file__).resolve().parents[4] / 'skills' / 'workflow-evolution' / 'SKILL.md'
    data = path.read_bytes()
    if not data or len(data) > 12000:
        raise ValueError('Missing or oversized workflow-evolution Skill')
    return data.decode('utf-8')


def identity():
    return hashlib.sha256(instructions().encode()).hexdigest()


def package():
    return {'version': '0.1.0', 'scope': {'kind': 'global'}, 'permissions': [],
            'files': [{'path': 'SKILL.md', 'content_base64': base64.b64encode(instructions().encode()).decode()}]}
